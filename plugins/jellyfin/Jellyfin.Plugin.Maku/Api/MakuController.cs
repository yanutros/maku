using System;
using System.Collections.Generic;
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
using MediaBrowser.Controller.Library;
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
/// <item>« /Maku/tracearr/api/v1/… » : l'API de Tracearr, transmise telle quelle (cookies de session compris) ;</item>
/// <item>« /Maku/maku/kavita… » et « /Maku/kavita/api/… » : un compte Kavita par utilisateur, et l'API de Kavita
/// (voir <see cref="KavitaAccounts"/>).</item>
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
    private readonly IUserManager _users;
    private readonly ILogger<MakuController> _logger;

    /// <summary>
    /// Initializes a new instance of the <see cref="MakuController"/> class.
    /// </summary>
    /// <param name="httpFactory">Clients HTTP de Jellyfin.</param>
    /// <param name="host">Le serveur (son identifiant).</param>
    /// <param name="users">Les utilisateurs du serveur (Admin › Livres).</param>
    /// <param name="logger">Journal.</param>
    public MakuController(IHttpClientFactory httpFactory, IApplicationHost host, IUserManager users, ILogger<MakuController> logger)
    {
        _httpFactory = httpFactory;
        _host = host;
        _users = users;
        _logger = logger;
    }

    /// <summary>Kavita est-il configuré (adresse et clé de l'administrateur) ?</summary>
    private static bool KavitaReady(PluginConfiguration c) => RelayRules.CleanBase(c.KavitaUrl).Length > 0 && c.KavitaApiKey.Length > 0;

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
            kavita = KavitaReady(c),
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

    /// <summary>
    /// La clé Kavita de l'utilisateur : celle de l'administrateur de Kavita pour un administrateur du serveur, sinon celle
    /// de son compte, s'il a au moins une bibliothèque ouverte.
    /// </summary>
    /// <param name="cancel">Annulation.</param>
    /// <returns>La clé, et s'il administre le serveur.</returns>
    [Authorize]
    [HttpGet("maku/kavita")]
    [Produces("application/json")]
    public async Task<ActionResult> KavitaSession(CancellationToken cancel)
    {
        var c = Config;
        if (!KavitaReady(c))
        {
            return Error(HttpStatusCode.NotFound, "not found");
        }

        var userId = JellyfinUserId();
        if (userId is null)
        {
            return Error(HttpStatusCode.Forbidden, "a user token is required");
        }

        var admin = IsAdmin();
        var key = KavitaAccounts.KeyFor(c, userId, admin);
        if (key is null)
        {
            return Error(HttpStatusCode.Forbidden, "no access");
        }

        var keys = await KavitaAccounts.KeysOf(_httpFactory.CreateClient(), RelayRules.CleanBase(c.KavitaUrl), key, cancel).ConfigureAwait(false);
        KavitaAccounts.Shared.MarkSeen(keys);
        return Ok(new { key, admin });
    }

    /// <summary>Bibliothèques Kavita, comptes déjà créés et utilisateurs du serveur (Maku › Admin › Livres).</summary>
    /// <param name="cancel">Annulation.</param>
    /// <returns>Les bibliothèques, les comptes et les utilisateurs.</returns>
    [Authorize(Policy = "RequiresElevation")]
    [HttpGet("maku/kavita/admin")]
    [Produces("application/json")]
    public async Task<ActionResult> KavitaAdmin(CancellationToken cancel)
    {
        var c = Config;
        if (!KavitaReady(c))
        {
            return Error(HttpStatusCode.NotFound, "not found");
        }

        IReadOnlyList<KavitaLibrary> libraries;
        try
        {
            libraries = await KavitaAccounts.Libraries(_httpFactory.CreateClient(), c, cancel).ConfigureAwait(false);
        }
        catch (Exception e) when (e is HttpRequestException or KavitaException or TaskCanceledException or System.Text.Json.JsonException)
        {
            return Error(HttpStatusCode.BadGateway, e.Message);
        }

        var me = RelayRules.NormalizeUserId(JellyfinUserId());
        var members = _users.GetUsers()
            .Select(u => new { id = "jellyfin:" + RelayRules.NormalizeUserId(u.Id.ToString("N")), name = u.Username })
            .Where(u => u.id != "jellyfin:" + me)
            .OrderBy(u => u.name, StringComparer.CurrentCultureIgnoreCase);
        var users = c.KavitaAccounts.Select(a => new
        {
            id = "jellyfin:" + RelayRules.NormalizeUserId(a.JellyfinUserId),
            name = a.Name,
            username = a.Username,
            libraries = a.Libraries,
        });
        return Ok(new
        {
            libraries = libraries.Select(l => new { id = l.Id, name = l.Name, type = l.Type }),
            users,
            members,
        });
    }

    /// <summary>Bibliothèques Kavita ouvertes à un utilisateur ; la première crée son compte Kavita.</summary>
    /// <param name="id">« jellyfin:&lt;identifiant&gt; ».</param>
    /// <param name="grant">Son nom et ses bibliothèques.</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>Le compte.</returns>
    [Authorize(Policy = "RequiresElevation")]
    [HttpPut("maku/kavita/admin/users/{id}")]
    [Produces("application/json")]
    public async Task<ActionResult> KavitaGrant([FromRoute] string id, [FromBody] KavitaGrantBody grant, CancellationToken cancel)
    {
        var c = Config;
        if (!KavitaReady(c) || Plugin.Instance is null)
        {
            return Error(HttpStatusCode.NotFound, "not found");
        }

        var userId = KavitaAccounts.GrantTarget(id, JellyfinUserId());
        if (userId is null)
        {
            return Error(HttpStatusCode.BadRequest, "bad user");
        }

        var name = (grant.Name ?? string.Empty).Trim();
        name = name.Length > 100 ? name[..100] : name;
        var libraries = (grant.Libraries ?? []).Distinct().Order().ToList();
        var http = _httpFactory.CreateClient();
        await KavitaAccounts.Shared.Writing.WaitAsync(cancel).ConfigureAwait(false);
        try
        {
            var account = c.KavitaAccounts.FirstOrDefault(a => RelayRules.NormalizeUserId(a.JellyfinUserId) == userId);
            if (account is null && libraries.Count == 0)
            {
                return Ok(new { id, name, libraries });
            }

            if (account is null)
            {
                account = await KavitaAccounts.Create(http, c, userId, name, libraries, cancel).ConfigureAwait(false);
                c.KavitaAccounts.Add(account);
            }
            else
            {
                if (!account.Libraries.SequenceEqual(libraries))
                {
                    await KavitaAccounts.SetLibraries(http, c, account, libraries, cancel).ConfigureAwait(false);
                }

                account.Libraries = libraries;
                if (name.Length > 0)
                {
                    account.Name = name;
                }
            }

            Plugin.Instance.SaveConfiguration();
            return Ok(new { id, name = account.Name, username = account.Username, libraries = account.Libraries });
        }
        catch (Exception e) when (e is HttpRequestException or KavitaException or TaskCanceledException or System.Text.Json.JsonException)
        {
            _logger.LogWarning(e, "Maku : Kavita a refusé la création ou la mise à jour d'un compte");
            return Error(HttpStatusCode.BadGateway, e.Message);
        }
        finally
        {
            KavitaAccounts.Shared.Writing.Release();
        }
    }

    /// <summary>
    /// API de Kavita : avec le jeton Jellyfin de l'utilisateur (les appels du moteur de Maku), ou avec une clé Kavita vue
    /// récemment auprès d'un utilisateur reconnu (les images, que la page charge sans en-tête). Kavita vérifie ensuite la
    /// clé : chacun ne voit que ce que son compte autorise.
    /// </summary>
    /// <param name="path">Chemin après « /Maku/kavita/ ».</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>La réponse de Kavita.</returns>
    [AllowAnonymous]
    [Route("kavita/{**path}")]
    [AcceptVerbs("GET", "POST", "PUT", "DELETE")]
    public async Task<ActionResult> Kavita(string? path, CancellationToken cancel)
    {
        var c = Config;
        var baseUrl = RelayRules.CleanBase(c.KavitaUrl);
        var apiPath = KavitaAccounts.ApiPath(path);
        if (!KavitaReady(c) || apiPath is null)
        {
            return Error(HttpStatusCode.NotFound, "not found");
        }

        var key = Request.Headers["x-api-key"].ToString();
        if (key.Length == 0)
        {
            key = Request.Query["apiKey"].ToString();
        }

        var recognized = JellyfinUserId() is not null;
        if (!KavitaAccounts.Shared.MayPass(recognized, key))
        {
            return Error(HttpStatusCode.Unauthorized, "not a user of this server");
        }

        // Clé d'un utilisateur reconnu : elle et ses voisines (celle des images d'un livre) ouvrent les images.
        if (recognized && key.Length > 0 && !KavitaAccounts.Shared.Fresh(key))
        {
            var keys = await KavitaAccounts.KeysOf(_httpFactory.CreateClient(), baseUrl, key, cancel).ConfigureAwait(false);
            KavitaAccounts.Shared.MarkSeen(keys);
        }

        var body = await ReadBody(cancel).ConfigureAwait(false);
        if (body is null)
        {
            return Error(HttpStatusCode.RequestEntityTooLarge, "request too large");
        }

        using var message = new HttpRequestMessage(new HttpMethod(Request.Method), baseUrl + "/" + apiPath + Request.QueryString.Value);
        // Seuls passent la clé Kavita et ce qui décrit la réponse attendue : jamais le jeton Jellyfin.
        foreach (var name in new[] { "x-api-key", "accept", "accept-language", "if-none-match", "if-modified-since", "range" })
        {
            if (Request.Headers.TryGetValue(name, out var values))
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

        try
        {
            using var response = await _httpFactory.CreateClient().SendAsync(message, HttpCompletionOption.ResponseHeadersRead, cancel).ConfigureAwait(false);
            return await Relay(response, keepCookies: false, cancel).ConfigureAwait(false);
        }
        catch (HttpRequestException e)
        {
            _logger.LogWarning(e, "Maku : Kavita injoignable ({Url})", baseUrl);
            return Error(HttpStatusCode.BadGateway, "kavita unreachable");
        }
    }

    /// <summary>L'utilisateur de l'appel administre-t-il le serveur ?</summary>
    private bool IsAdmin() => User.IsInRole("Administrator");

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

/// <summary>Bibliothèques Kavita ouvertes à un utilisateur (Maku › Admin › Livres).</summary>
public class KavitaGrantBody
{
    /// <summary>Gets or sets le nom de l'utilisateur, tel que l'administrateur le voit.</summary>
    public string? Name { get; set; }

    /// <summary>Gets or sets les bibliothèques Kavita ouvertes.</summary>
    public List<int>? Libraries { get; set; }
}
