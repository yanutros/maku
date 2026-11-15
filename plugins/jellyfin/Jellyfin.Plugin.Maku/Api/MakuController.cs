using System;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Threading;
using System.Threading.Tasks;
using Jellyfin.Plugin.Maku.Configuration;
using Jellyfin.Plugin.Maku.Relay;
using MediaBrowser.Common;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Logging;

namespace Jellyfin.Plugin.Maku.Api;

/// <summary>
/// Le relais de Maku dans Jellyfin, à l'adresse du serveur :
/// <list type="bullet">
/// <item>« GET /Maku/maku/info » : ce que le plugin propose (sans jeton), pour que Maku le reconnaisse ;</item>
/// <item>« /Maku/seerr/api/v1/… » : l'API de Seerr, au nom du compte Seerr de l'utilisateur (clé d'API du plugin) ;</item>
/// <item>« /Maku/tracearr/api/v1/… » : l'API de Tracearr, transmise telle quelle (cookies de session compris).</item>
/// </list>
/// Seuls les utilisateurs du serveur passent : Jellyfin vérifie leur jeton avant d'appeler le relais.
/// </summary>
[ApiController]
[Route("Maku")]
public class MakuController : ControllerBase
{
    /// <summary>Taille maximale d'un envoi (demandes, réglages : tout petits).</summary>
    private const int MaxBody = 4 * 1024 * 1024;

    private readonly IHttpClientFactory _httpFactory;
    private readonly IApplicationHost _host;
    private readonly ILogger<MakuController> _logger;

    /// <summary>
    /// Initializes a new instance of the <see cref="MakuController"/> class.
    /// </summary>
    /// <param name="httpFactory">Clients HTTP de Jellyfin.</param>
    /// <param name="host">Le serveur (son identifiant).</param>
    /// <param name="logger">Journal.</param>
    public MakuController(IHttpClientFactory httpFactory, IApplicationHost host, ILogger<MakuController> logger)
    {
        _httpFactory = httpFactory;
        _host = host;
        _logger = logger;
    }

    private static PluginConfiguration Config => Plugin.Instance?.Configuration ?? new PluginConfiguration();

    /// <summary>Ce que le plugin propose, lu par Maku pour le reconnaître (même format que Maku Server).</summary>
    /// <returns>Le type et l'identifiant du serveur, et les services relayés.</returns>
    [HttpGet("maku/info")]
    [AllowAnonymous]
    [Produces("application/json")]
    public ActionResult Info()
    {
        var c = Config;
        return Ok(new
        {
            app = "maku-server",
            plugin = true,
            version = typeof(Plugin).Assembly.GetName().Version?.ToString(3),
            server = "jellyfin",
            id = _host.SystemId,
            seerr = RelayRules.CleanBase(c.SeerrUrl).Length > 0 && c.SeerrApiKey.Length > 0,
            tracearr = RelayRules.CleanBase(c.TracearrUrl).Length > 0,
        });
    }

    /// <summary>API de Seerr, au nom du compte Seerr de l'utilisateur.</summary>
    /// <param name="path">Chemin après « /Maku/seerr/ ».</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>La réponse de Seerr.</returns>
    [Authorize]
    [Route("seerr/{**path}")]
    [AcceptVerbs("GET", "POST", "PUT", "PATCH", "DELETE")]
    public async Task<ActionResult> Seerr(string? path, CancellationToken cancel)
    {
        var c = Config;
        var baseUrl = RelayRules.CleanBase(c.SeerrUrl);
        var apiPath = RelayRules.ApiPath(path);
        if (baseUrl.Length == 0 || c.SeerrApiKey.Length == 0 || apiPath is null)
        {
            return Error(HttpStatusCode.NotFound, "not found");
        }

        var userId = JellyfinUserId();
        if (userId is null)
        {
            return Error(HttpStatusCode.Forbidden, "a user token is required");
        }

        var http = _httpFactory.CreateClient();
        int? account;
        try
        {
            account = await SeerrAccounts.Shared.AccountFor(http, c, userId, cancel).ConfigureAwait(false);
        }
        catch (Exception e) when (e is HttpRequestException or TaskCanceledException or System.Text.Json.JsonException)
        {
            _logger.LogWarning(e, "Maku : Seerr injoignable ou clé d'API refusée ({Url})", baseUrl);
            return Error(HttpStatusCode.BadGateway, "seerr unreachable");
        }

        if (account is null)
        {
            return Error(HttpStatusCode.Forbidden, "no seerr account for this user");
        }

        var body = await ReadBody(cancel).ConfigureAwait(false);
        if (body is null)
        {
            return Error(HttpStatusCode.RequestEntityTooLarge, "request too large");
        }

        var url = baseUrl + "/" + apiPath + Request.QueryString.Value;
        var unsafeMethod = !HttpMethods.IsGet(Request.Method) && !HttpMethods.IsHead(Request.Method);
        HttpResponseMessage? response = null;
        try
        {
            // Seerr qui protège ses formulaires : un jeton refusé est redemandé une fois.
            for (var attempt = 0; attempt < 2; attempt++)
            {
                using var message = Upstream(url, body, keepCookies: false);
                message.Headers.TryAddWithoutValidation("X-Api-Key", c.SeerrApiKey);
                message.Headers.TryAddWithoutValidation("X-Api-User", account.Value.ToString(System.Globalization.CultureInfo.InvariantCulture));
                if (unsafeMethod)
                {
                    var csrf = await SeerrAccounts.Shared.Csrf(http, baseUrl, attempt > 0, cancel).ConfigureAwait(false);
                    if (csrf is { } t)
                    {
                        message.Headers.TryAddWithoutValidation("Cookie", t.Cookies);
                        message.Headers.TryAddWithoutValidation("X-XSRF-TOKEN", t.Token);
                    }
                }

                response?.Dispose();
                response = await http.SendAsync(message, HttpCompletionOption.ResponseHeadersRead, cancel).ConfigureAwait(false);
                if (!(unsafeMethod && attempt == 0 && response.StatusCode == HttpStatusCode.Forbidden && await MentionsCsrf(response, cancel).ConfigureAwait(false)))
                {
                    break;
                }
            }

            return await Relay(response!, keepCookies: false, cancel).ConfigureAwait(false);
        }
        catch (HttpRequestException e)
        {
            _logger.LogWarning(e, "Maku : Seerr injoignable ({Url})", baseUrl);
            return Error(HttpStatusCode.BadGateway, "seerr unreachable");
        }
        finally
        {
            response?.Dispose();
        }
    }

    /// <summary>API de Tracearr, transmise telle quelle : la session de l'utilisateur tient dans ses cookies.</summary>
    /// <param name="path">Chemin après « /Maku/tracearr/ ».</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>La réponse de Tracearr.</returns>
    [Authorize]
    [Route("tracearr/{**path}")]
    [AcceptVerbs("GET", "POST", "PUT", "PATCH", "DELETE")]
    public async Task<ActionResult> Tracearr(string? path, CancellationToken cancel)
    {
        var baseUrl = RelayRules.CleanBase(Config.TracearrUrl);
        var apiPath = RelayRules.ApiPath(path);
        if (baseUrl.Length == 0 || apiPath is null)
        {
            return Error(HttpStatusCode.NotFound, "not found");
        }

        if (JellyfinUserId() is null)
        {
            return Error(HttpStatusCode.Forbidden, "a user token is required");
        }

        var body = await ReadBody(cancel).ConfigureAwait(false);
        if (body is null)
        {
            return Error(HttpStatusCode.RequestEntityTooLarge, "request too large");
        }

        using var message = Upstream(baseUrl + "/" + apiPath + Request.QueryString.Value, body, keepCookies: true);
        // Tracearr vérifie l'origine des requêtes qui ouvrent une session : c'est désormais la sienne.
        if (Request.Headers.ContainsKey("Origin"))
        {
            message.Headers.TryAddWithoutValidation("Origin", new Uri(baseUrl).GetLeftPart(UriPartial.Authority));
        }

        try
        {
            using var response = await _httpFactory.CreateClient().SendAsync(message, HttpCompletionOption.ResponseHeadersRead, cancel).ConfigureAwait(false);
            return await Relay(response, keepCookies: true, cancel).ConfigureAwait(false);
        }
        catch (HttpRequestException e)
        {
            _logger.LogWarning(e, "Maku : Tracearr injoignable ({Url})", baseUrl);
            return Error(HttpStatusCode.BadGateway, "tracearr unreachable");
        }
    }

    /// <summary>Comptes de Seerr, pour lier un utilisateur Jellyfin à la main (page de réglages, administrateur).</summary>
    /// <param name="cancel">Annulation.</param>
    /// <returns>Les comptes de Seerr.</returns>
    [Authorize(Policy = "RequiresElevation")]
    [HttpGet("admin/seerr-users")]
    [Produces("application/json")]
    public async Task<ActionResult> SeerrUsers(CancellationToken cancel)
    {
        var c = Config;
        if (RelayRules.CleanBase(c.SeerrUrl).Length == 0 || c.SeerrApiKey.Length == 0)
        {
            return Error(HttpStatusCode.NotFound, "seerr is not set up");
        }

        try
        {
            var accounts = await SeerrAccounts.Shared.Accounts(_httpFactory.CreateClient(), c, true, cancel).ConfigureAwait(false);
            return Ok(accounts.Select(a => new { id = a.Id, name = a.Name, jellyfinUserId = a.JellyfinUserId }));
        }
        catch (Exception e) when (e is HttpRequestException or TaskCanceledException or System.Text.Json.JsonException)
        {
            return Error(HttpStatusCode.BadGateway, e.Message);
        }
    }

    /// <summary>Utilisateur Jellyfin de l'appel (un jeton de clé d'API n'en a pas).</summary>
    private string? JellyfinUserId()
    {
        if (string.Equals(User.FindFirst("Jellyfin-IsApiKey")?.Value, "true", StringComparison.OrdinalIgnoreCase))
        {
            return null;
        }

        var id = User.FindFirst("Jellyfin-UserId")?.Value;
        return string.IsNullOrEmpty(id) || Guid.TryParse(id, out var g) && g == Guid.Empty ? null : id;
    }

    /// <summary>Corps de l'appel (null s'il dépasse la taille permise).</summary>
    private async Task<byte[]?> ReadBody(CancellationToken cancel)
    {
        if (Request.ContentLength > MaxBody)
        {
            return null;
        }

        using var copy = new MemoryStream();
        var buffer = new byte[81920];
        int read;
        while ((read = await Request.Body.ReadAsync(buffer, cancel).ConfigureAwait(false)) > 0)
        {
            copy.Write(buffer, 0, read);
            if (copy.Length > MaxBody)
            {
                return null;
            }
        }

        return copy.ToArray();
    }

    /// <summary>Requête vers le service : même méthode et même corps, et les seuls en-têtes permis.</summary>
    private HttpRequestMessage Upstream(string url, byte[] body, bool keepCookies)
    {
        var message = new HttpRequestMessage(new HttpMethod(Request.Method), url);
        foreach (var (name, values) in Request.Headers)
        {
            if (RelayRules.Forwarded(name, keepCookies))
            {
                message.Headers.TryAddWithoutValidation(name, values.ToArray());
            }
        }

        if (body.Length > 0)
        {
            message.Content = new ByteArrayContent(body);
            if (Request.ContentType is { Length: > 0 } type && MediaTypeHeaderValue.TryParse(type, out var parsed))
            {
                message.Content.Headers.ContentType = parsed;
            }
        }

        message.Headers.TryAddWithoutValidation("X-Forwarded-For", HttpContext.Connection.RemoteIpAddress?.ToString());
        return message;
    }

    /// <summary>Rend la réponse du service à Maku : statut, en-têtes permis (cookies de Tracearr compris) et corps.</summary>
    private async Task<ActionResult> Relay(HttpResponseMessage response, bool keepCookies, CancellationToken cancel)
    {
        Response.StatusCode = (int)response.StatusCode;
        foreach (var (name, values) in response.Headers.Concat(response.Content.Headers))
        {
            if (keepCookies && name.Equals("set-cookie", StringComparison.OrdinalIgnoreCase))
            {
                Response.Headers.Append(name, values.Select(RelayRules.StripDomain).ToArray());
            }
            else if (RelayRules.Returned(name) || name.Equals("content-type", StringComparison.OrdinalIgnoreCase))
            {
                Response.Headers[name] = values.ToArray();
            }
        }

        await response.Content.CopyToAsync(Response.Body, cancel).ConfigureAwait(false);
        return new EmptyResult();
    }

    /// <summary>Seerr a-t-il refusé l'envoi pour son jeton CSRF ?</summary>
    private static async Task<bool> MentionsCsrf(HttpResponseMessage response, CancellationToken cancel)
    {
        var text = await response.Content.ReadAsStringAsync(cancel).ConfigureAwait(false);
        return text.Contains("csrf", StringComparison.OrdinalIgnoreCase);
    }

    private ObjectResult Error(HttpStatusCode status, string message)
        => StatusCode((int)status, new { error = message });
}
