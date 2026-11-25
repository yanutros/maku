using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using System.Web;
using Jellyfin.Plugin.Maku.Configuration;

namespace Jellyfin.Plugin.Maku.Relay;

/// <summary>Une bibliothèque Kavita (son type : 0 manga, 1 BD, 2 livre, 4 light novel, 5 comics).</summary>
/// <param name="Id">Numéro de la bibliothèque.</param>
/// <param name="Name">Nom affiché.</param>
/// <param name="Type">Type Kavita (LibraryType).</param>
public sealed record KavitaLibrary(int Id, string Name, int Type);

/// <summary>Kavita a refusé un appel, ou n'a pas répondu comme prévu.</summary>
public sealed class KavitaException : Exception
{
    /// <summary>
    /// Initializes a new instance of the <see cref="KavitaException"/> class.
    /// </summary>
    /// <param name="message">Ce qui s'est passé.</param>
    public KavitaException(string message)
        : base(message)
    {
    }

    /// <summary>
    /// Initializes a new instance of the <see cref="KavitaException"/> class.
    /// </summary>
    public KavitaException()
    {
    }

    /// <summary>
    /// Initializes a new instance of the <see cref="KavitaException"/> class.
    /// </summary>
    /// <param name="message">Ce qui s'est passé.</param>
    /// <param name="inner">La cause.</param>
    public KavitaException(string message, Exception inner)
        : base(message, inner)
    {
    }
}

/// <summary>
/// Kavita (livres, BD, mangas) : un compte Kavita par utilisateur Jellyfin, créé et réglé avec la clé d'API de
/// l'administrateur de Kavita (réglages du plugin), qui ne quitte jamais le serveur. Chacun lit ensuite avec son propre
/// compte : Kavita refuse ce qui ne lui est pas ouvert, et la progression de lecture est la sienne. Même fonctionnement
/// que Maku Server (server/src/kavita.rs).
/// </summary>
public sealed class KavitaAccounts : IDisposable
{
    /// <summary>Une clé vue auprès d'un utilisateur reconnu ouvre les images (chargées sans en-tête) pendant un jour.</summary>
    public static readonly TimeSpan SeenFor = TimeSpan.FromDays(1);

    /// <summary>Sans limite d'âge (AgeRating.NotApplicable de Kavita).</summary>
    private const int NoAgeLimit = -1;

    /// <summary>Rôle Kavita qui autorise la connexion (avec « Pleb », que Kavita ajoute lui-même).</summary>
    private const string LoginRole = "Login";

    private readonly ConcurrentDictionary<string, DateTime> _seen = new(StringComparer.Ordinal);
    private readonly SemaphoreSlim _writing = new(1, 1);

    /// <summary>Gets l'instance partagée par tous les appels.</summary>
    public static KavitaAccounts Shared { get; } = new();

    /// <summary>Gets une modification des comptes à la fois (création, bibliothèques).</summary>
    public SemaphoreSlim Writing => _writing;

    /// <inheritdoc />
    public void Dispose() => _writing.Dispose();

    /// <summary>Note des clés comme vues maintenant, auprès d'un utilisateur reconnu.</summary>
    /// <param name="keys">Clés Kavita.</param>
    /// <param name="now">Heure (pour les tests).</param>
    public void MarkSeen(IEnumerable<string> keys, DateTime? now = null)
    {
        var at = now ?? DateTime.UtcNow;
        if (_seen.Count > 5000)
        {
            foreach (var old in _seen.Where(p => at - p.Value >= SeenFor).Select(p => p.Key).ToList())
            {
                _seen.TryRemove(old, out _);
            }
        }

        foreach (var k in keys.Where(k => k.Length > 0))
        {
            _seen[k] = at;
        }
    }

    /// <summary>Clé vue depuis moins d'un jour auprès d'un utilisateur reconnu ?</summary>
    /// <param name="key">Clé Kavita.</param>
    /// <param name="now">Heure (pour les tests).</param>
    /// <returns>Vrai si elle ouvre les images.</returns>
    public bool Fresh(string key, DateTime? now = null)
        => key.Length > 0 && _seen.TryGetValue(key, out var at) && (now ?? DateTime.UtcNow) - at < SeenFor;

    /// <summary>
    /// Appel de l'API de Kavita par le relais : avec le jeton Jellyfin d'un utilisateur (les appels du moteur de Maku), ou
    /// avec une clé vue depuis moins d'un jour (les images, que la page charge sans en-tête).
    /// </summary>
    /// <param name="recognized">Jellyfin a reconnu l'utilisateur de l'appel.</param>
    /// <param name="key">Clé Kavita de l'appel (en-tête ou paramètre « apiKey »).</param>
    /// <param name="now">Heure (pour les tests).</param>
    /// <returns>Vrai si l'appel passe ; Kavita vérifie ensuite la clé.</returns>
    public bool MayPass(bool recognized, string key, DateTime? now = null) => recognized || Fresh(key, now);

    /// <summary>
    /// Clé Kavita d'un utilisateur : celle de l'administrateur de Kavita pour un administrateur du serveur, sinon celle de
    /// son compte, s'il a au moins une bibliothèque ouverte. Null : pas d'accès.
    /// </summary>
    /// <param name="config">Réglages du plugin.</param>
    /// <param name="jellyfinUserId">Utilisateur Jellyfin.</param>
    /// <param name="admin">Il administre le serveur.</param>
    /// <returns>La clé, ou null.</returns>
    public static string? KeyFor(PluginConfiguration config, string jellyfinUserId, bool admin)
    {
        if (admin)
        {
            return config.KavitaApiKey.Length > 0 ? config.KavitaApiKey : null;
        }

        var id = RelayRules.NormalizeUserId(jellyfinUserId);
        return id.Length == 0 ? null : config.KavitaAccounts.FirstOrDefault(a => RelayRules.NormalizeUserId(a.JellyfinUserId) == id && a.Libraries.Count > 0)?.Key;
    }

    /// <summary>
    /// Utilisateur à qui l'administrateur ouvre des bibliothèques : « jellyfin:&lt;identifiant&gt; », un utilisateur de ce
    /// serveur, et jamais l'administrateur lui-même (il lit avec le compte administrateur de Kavita).
    /// </summary>
    /// <param name="id">Identifiant reçu.</param>
    /// <param name="me">L'administrateur qui fait la demande.</param>
    /// <returns>L'identifiant Jellyfin (32 caractères hexadécimaux), ou null s'il est refusé.</returns>
    public static string? GrantTarget(string? id, string? me)
    {
        if (id is null || !id.StartsWith("jellyfin:", StringComparison.Ordinal))
        {
            return null;
        }

        var raw = id["jellyfin:".Length..];
        var userId = RelayRules.NormalizeUserId(raw);
        var hex = userId.Length == 32 && userId.All(char.IsAsciiHexDigitLower) && raw.All(c => char.IsAsciiHexDigit(c) || c == '-');
        return hex && userId != RelayRules.NormalizeUserId(me) ? userId : null;
    }

    /// <summary>Toutes les clés d'un compte : celle de Maku, et celle que Kavita met dans les adresses des images d'un livre.</summary>
    /// <param name="http">Client HTTP.</param>
    /// <param name="baseUrl">Adresse de Kavita.</param>
    /// <param name="key">Une clé du compte.</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>Les clés.</returns>
    public static async Task<IReadOnlyList<string>> KeysOf(HttpClient http, string baseUrl, string key, CancellationToken cancel)
    {
        var keys = new List<string> { key };
        try
        {
            using var doc = await Call(http, HttpMethod.Get, baseUrl, "/api/Account/auth-keys", key, null, null, cancel).ConfigureAwait(false);
            if (doc?.RootElement.ValueKind == JsonValueKind.Array)
            {
                keys.AddRange(doc.RootElement.EnumerateArray().Select(k => Text(k, "key")).OfType<string>());
            }
        }
        catch (Exception e) when (e is HttpRequestException or KavitaException or TaskCanceledException or JsonException)
        {
            // La clé seule suffit à ce que l'API passe.
        }

        return keys;
    }

    /// <summary>Bibliothèques de Kavita (clé de l'administrateur).</summary>
    /// <param name="http">Client HTTP.</param>
    /// <param name="config">Réglages du plugin.</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>Les bibliothèques.</returns>
    public static async Task<IReadOnlyList<KavitaLibrary>> Libraries(HttpClient http, PluginConfiguration config, CancellationToken cancel)
    {
        using var doc = await Call(http, HttpMethod.Get, RelayRules.CleanBase(config.KavitaUrl), "/api/Library/libraries", config.KavitaApiKey, null, null, cancel).ConfigureAwait(false);
        if (doc?.RootElement.ValueKind != JsonValueKind.Array)
        {
            return [];
        }

        return doc.RootElement.EnumerateArray()
            .Where(l => Int(l, "id") is not null)
            .Select(l => new KavitaLibrary(Int(l, "id")!.Value, Text(l, "name") ?? string.Empty, Int(l, "type") ?? 1))
            .ToList();
    }

    /// <summary>
    /// Crée le compte Kavita d'un utilisateur : invitation (sans e-mail configuré, Kavita rend le lien d'activation),
    /// activation avec un mot de passe tiré au hasard, puis une clé d'API pour Maku.
    /// </summary>
    /// <param name="http">Client HTTP.</param>
    /// <param name="config">Réglages du plugin.</param>
    /// <param name="jellyfinUserId">Utilisateur Jellyfin (32 caractères hexadécimaux).</param>
    /// <param name="name">Son nom.</param>
    /// <param name="libraries">Bibliothèques ouvertes.</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>Le compte créé.</returns>
    public static async Task<KavitaAccount> Create(HttpClient http, PluginConfiguration config, string jellyfinUserId, string name, IReadOnlyList<int> libraries, CancellationToken cancel)
    {
        var baseUrl = RelayRules.CleanBase(config.KavitaUrl);
        var email = $"maku-jellyfin-{jellyfinUserId}@maku.invalid";
        var username = UsernameFor(name, jellyfinUserId);
        var invite = new
        {
            email,
            roles = new[] { LoginRole },
            libraries,
            ageRestriction = new { ageRating = NoAgeLimit, includeUnknowns = true },
        };
        string link;
        try
        {
            using var doc = await Call(http, HttpMethod.Post, baseUrl, "/api/Account/invite", config.KavitaApiKey, null, invite, cancel).ConfigureAwait(false);
            link = (doc is null ? null : Text(doc.RootElement, "emailLink")) ?? throw new KavitaException("Kavita : invitation sans lien d'activation");
        }
        catch (KavitaException e) when (e.Message.Contains("already", StringComparison.OrdinalIgnoreCase))
        {
            // Une création interrompue : le compte existe, pas encore activé ; on reprend son lien.
            link = await PendingLink(http, baseUrl, config.KavitaApiKey, email, cancel).ConfigureAwait(false);
        }

        var token = Uri.TryCreate(link, UriKind.Absolute, out var uri) ? HttpUtility.ParseQueryString(uri.Query)["token"] : null;
        if (string.IsNullOrEmpty(token))
        {
            throw new KavitaException("Kavita : lien d'activation sans jeton");
        }

        var confirm = new { email, token, password = RandomHex(24), username };
        int kavitaId;
        string jwt;
        using (var user = await Call(http, HttpMethod.Post, baseUrl, "/api/Account/confirm-email", null, null, confirm, cancel).ConfigureAwait(false))
        {
            kavitaId = (user is null ? null : Int(user.RootElement, "id")) ?? throw new KavitaException("Kavita : compte activé sans numéro");
            jwt = (user is null ? null : Text(user.RootElement, "token")) ?? throw new KavitaException("Kavita : compte activé sans jeton");
        }

        using var created = await Call(http, HttpMethod.Post, baseUrl, "/api/Account/create-auth-key", null, jwt, new { keyLength = 32, name = "Maku" }, cancel).ConfigureAwait(false);
        var key = (created is null ? null : Text(created.RootElement, "key")) ?? throw new KavitaException("Kavita : clé d'API absente");
        return new KavitaAccount
        {
            JellyfinUserId = jellyfinUserId,
            Name = name,
            KavitaId = kavitaId,
            Username = username,
            Key = key,
            Libraries = libraries.ToList(),
        };
    }

    /// <summary>Change les bibliothèques ouvertes à un compte (vide : plus d'accès, le compte et sa progression restent).</summary>
    /// <param name="http">Client HTTP.</param>
    /// <param name="config">Réglages du plugin.</param>
    /// <param name="account">Le compte.</param>
    /// <param name="libraries">Bibliothèques ouvertes.</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>Une tâche.</returns>
    public static async Task SetLibraries(HttpClient http, PluginConfiguration config, KavitaAccount account, IReadOnlyList<int> libraries, CancellationToken cancel)
    {
        var body = new
        {
            userId = account.KavitaId,
            username = account.Username,
            roles = new[] { LoginRole },
            libraries,
            ageRestriction = new { ageRating = NoAgeLimit, includeUnknowns = true },
            email = (string?)null,
            identityProvider = 0,
        };
        using var _ = await Call(http, HttpMethod.Post, RelayRules.CleanBase(config.KavitaUrl), "/api/Account/update", config.KavitaApiKey, null, body, cancel).ConfigureAwait(false);
    }

    /// <summary>
    /// Nom du compte Kavita : celui de l'utilisateur (lettres sans accents, chiffres, « - », « . », « _ »), puis la fin de
    /// son identifiant, pour que deux homonymes ne se gênent pas.
    /// </summary>
    /// <param name="name">Nom de l'utilisateur.</param>
    /// <param name="id">Son identifiant Jellyfin.</param>
    /// <returns>Le nom du compte.</returns>
    public static string UsernameFor(string name, string id)
    {
        var folded = new StringBuilder();
        foreach (var c in (name ?? string.Empty).Normalize(NormalizationForm.FormD))
        {
            if (CharUnicodeInfo.GetUnicodeCategory(c) == UnicodeCategory.NonSpacingMark)
            {
                continue;
            }

            folded.Append(c is >= 'a' and <= 'z' or >= 'A' and <= 'Z' or >= '0' and <= '9' or '-' or '.' or '_' ? c : '-');
        }

        var b = folded.ToString().Trim('-', '.');
        b = b.Length > 24 ? b[..24] : b;
        if (b.Length == 0)
        {
            b = "lecteur";
        }

        var alnum = new string((id ?? string.Empty).Where(char.IsAsciiLetterOrDigit).ToArray());
        return b + "." + (alnum.Length > 6 ? alnum[^6..] : alnum);
    }

    /// <summary>Chemin Kavita à transmettre (« api/… ») : l'API seulement, et rien qui remonte d'un dossier, même encodé.</summary>
    /// <param name="path">Chemin après « /Maku/kavita/ ».</param>
    /// <returns>Le chemin, ou null s'il est refusé.</returns>
    public static string? ApiPath(string? path)
    {
        var p = (path ?? string.Empty).TrimStart('/');
        var lower = p.ToLowerInvariant();
        var escapes = lower.Contains("..", StringComparison.Ordinal) || lower.Contains("%2e", StringComparison.Ordinal)
            || lower.Contains('\\', StringComparison.Ordinal) || lower.Contains("%5c", StringComparison.Ordinal)
            || lower.Contains("%2f", StringComparison.Ordinal);
        return lower.StartsWith("api/", StringComparison.Ordinal) && !escapes ? p : null;
    }

    /// <summary>Lien d'activation d'un compte invité mais pas encore activé.</summary>
    private static async Task<string> PendingLink(HttpClient http, string baseUrl, string adminKey, string email, CancellationToken cancel)
    {
        int? id = null;
        using (var users = await Call(http, HttpMethod.Get, baseUrl, "/api/Users?includePending=true", adminKey, null, null, cancel).ConfigureAwait(false))
        {
            if (users?.RootElement.ValueKind == JsonValueKind.Array)
            {
                id = users.RootElement.EnumerateArray()
                    .Where(u => string.Equals(Text(u, "email"), email, StringComparison.OrdinalIgnoreCase))
                    .Select(u => Int(u, "id"))
                    .FirstOrDefault();
            }
        }

        if (id is null)
        {
            throw new KavitaException($"Kavita : un compte {email} existe déjà");
        }

        using var link = await Call(http, HttpMethod.Get, baseUrl, $"/api/Account/invite-url?userId={id}&withBaseUrl=true", adminKey, null, null, cancel).ConfigureAwait(false);
        return link?.RootElement.ValueKind == JsonValueKind.String ? link.RootElement.GetString()! : throw new KavitaException("Kavita : lien d'activation introuvable");
    }

    /// <summary>Appel à Kavita, avec une clé d'API ou un jeton de session ; null si la réponse est vide.</summary>
    private static async Task<JsonDocument?> Call(HttpClient http, HttpMethod method, string baseUrl, string path, string? key, string? bearer, object? body, CancellationToken cancel)
    {
        if (baseUrl.Length == 0)
        {
            throw new KavitaException("Kavita n'est pas configuré");
        }

        using var request = new HttpRequestMessage(method, baseUrl + path);
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
        if (key is { Length: > 0 })
        {
            request.Headers.TryAddWithoutValidation("x-api-key", key);
        }

        if (bearer is { Length: > 0 })
        {
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", bearer);
        }

        if (body is not null)
        {
            request.Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json");
        }

        using var response = await http.SendAsync(request, cancel).ConfigureAwait(false);
        var text = await response.Content.ReadAsStringAsync(cancel).ConfigureAwait(false);
        if (!response.IsSuccessStatusCode)
        {
            var detail = text.Length > 300 ? text[..300] : text;
            throw new KavitaException($"Kavita ({path}) : {(int)response.StatusCode} {detail}");
        }

        if (string.IsNullOrWhiteSpace(text))
        {
            return null;
        }

        try
        {
            return JsonDocument.Parse(text);
        }
        catch (JsonException)
        {
            // Une chaîne nue (lien d'invitation) : rendue comme chaîne JSON.
            return JsonDocument.Parse(JsonSerializer.Serialize(text));
        }
    }

    private static string? Text(JsonElement e, string name)
        => e.ValueKind == JsonValueKind.Object && e.TryGetProperty(name, out var p) && p.ValueKind == JsonValueKind.String && p.GetString() is { Length: > 0 } s ? s : null;

    private static int? Int(JsonElement e, string name)
        => e.ValueKind == JsonValueKind.Object && e.TryGetProperty(name, out var p) && p.ValueKind == JsonValueKind.Number && p.TryGetInt32(out var i) ? i : null;

    private static string RandomHex(int bytes) => Convert.ToHexStringLower(RandomNumberGenerator.GetBytes(bytes));
}
