using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Jellyfin.Plugin.Maku.Configuration;

namespace Jellyfin.Plugin.Maku.Relay;

/// <summary>Un compte Seerr, pour la page de réglages (liaison à la main).</summary>
/// <param name="Id">Numéro du compte.</param>
/// <param name="Name">Nom affiché.</param>
/// <param name="JellyfinUserId">Utilisateur Jellyfin d'où il a été importé, s'il y en a un.</param>
public sealed record SeerrAccount(int Id, string Name, string? JellyfinUserId);

/// <summary>
/// Ce que le relais garde de Seerr : ses comptes (pour retrouver celui d'un utilisateur Jellyfin) et, si Seerr protège
/// ses formulaires (CSRF), le jeton à joindre aux envois.
/// </summary>
public sealed class SeerrAccounts : IDisposable
{
    private static readonly TimeSpan AccountsLifetime = TimeSpan.FromMinutes(5);
    private static readonly TimeSpan RetryAfterMiss = TimeSpan.FromSeconds(30);

    private readonly SemaphoreSlim _gate = new(1, 1);
    private IReadOnlyList<SeerrAccount> _accounts = [];
    private DateTime _loadedAt = DateTime.MinValue;
    private string _loadedFor = string.Empty;
    private string? _csrfCookies;
    private string? _xsrfToken;

    /// <summary>Gets l'instance partagée par tous les appels.</summary>
    public static SeerrAccounts Shared { get; } = new();

    /// <summary>
    /// Compte Seerr sous lequel l'utilisateur Jellyfin agit : celui choisi par l'administrateur, sinon celui importé de
    /// Jellyfin (même identifiant). Null : aucun.
    /// </summary>
    /// <param name="http">Client HTTP.</param>
    /// <param name="config">Réglages du plugin.</param>
    /// <param name="jellyfinUserId">Utilisateur Jellyfin.</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>Le numéro du compte Seerr.</returns>
    public async Task<int?> AccountFor(HttpClient http, PluginConfiguration config, string jellyfinUserId, CancellationToken cancel)
    {
        var id = RelayRules.NormalizeUserId(jellyfinUserId);
        var link = config.UserLinks.FirstOrDefault(l => RelayRules.NormalizeUserId(l.JellyfinUserId) == id && l.SeerrUserId > 0);
        if (link is not null)
        {
            return link.SeerrUserId;
        }

        var found = Find(await Accounts(http, config, false, cancel).ConfigureAwait(false), id);
        // Utilisateur tout juste importé dans Seerr : la liste est relue, au plus toutes les 30 secondes.
        if (found is null && DateTime.UtcNow - _loadedAt > RetryAfterMiss)
        {
            found = Find(await Accounts(http, config, true, cancel).ConfigureAwait(false), id);
        }

        return found;
    }

    /// <summary>Comptes de Seerr, relus toutes les 5 minutes (ou tout de suite avec <paramref name="refresh"/>).</summary>
    /// <param name="http">Client HTTP.</param>
    /// <param name="config">Réglages du plugin.</param>
    /// <param name="refresh">Relire sans attendre.</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>Les comptes.</returns>
    public async Task<IReadOnlyList<SeerrAccount>> Accounts(HttpClient http, PluginConfiguration config, bool refresh, CancellationToken cancel)
    {
        var baseUrl = RelayRules.CleanBase(config.SeerrUrl);
        var key = baseUrl + "|" + config.SeerrApiKey;
        await _gate.WaitAsync(cancel).ConfigureAwait(false);
        try
        {
            if (!refresh && _loadedFor == key && DateTime.UtcNow - _loadedAt < AccountsLifetime)
            {
                return _accounts;
            }

            using var request = new HttpRequestMessage(HttpMethod.Get, baseUrl + "/api/v1/user?take=1000&skip=0");
            request.Headers.Add("X-Api-Key", config.SeerrApiKey);
            using var response = await http.SendAsync(request, cancel).ConfigureAwait(false);
            response.EnsureSuccessStatusCode();
            using var doc = JsonDocument.Parse(await response.Content.ReadAsStringAsync(cancel).ConfigureAwait(false));
            _accounts = Parse(doc.RootElement);
            _loadedAt = DateTime.UtcNow;
            _loadedFor = key;
            return _accounts;
        }
        finally
        {
            _gate.Release();
        }
    }

    /// <summary>Comptes lus dans la réponse de « GET /api/v1/user ».</summary>
    /// <param name="root">Corps de la réponse.</param>
    /// <returns>Les comptes.</returns>
    public static IReadOnlyList<SeerrAccount> Parse(JsonElement root)
    {
        var list = new List<SeerrAccount>();
        if (!root.TryGetProperty("results", out var results) || results.ValueKind != JsonValueKind.Array)
        {
            return list;
        }

        foreach (var u in results.EnumerateArray())
        {
            if (!u.TryGetProperty("id", out var idProp) || !idProp.TryGetInt32(out var id))
            {
                continue;
            }

            string? Text(string name) => u.TryGetProperty(name, out var p) && p.ValueKind == JsonValueKind.String && p.GetString() is { Length: > 0 } s ? s : null;
            var name = Text("displayName") ?? Text("username") ?? Text("jellyfinUsername") ?? Text("plexUsername") ?? Text("email") ?? ("#" + id);
            list.Add(new SeerrAccount(id, name, Text("jellyfinUserId")));
        }

        return list;
    }

    /// <summary>Compte importé de cet utilisateur Jellyfin.</summary>
    /// <param name="accounts">Comptes de Seerr.</param>
    /// <param name="jellyfinUserId">Identifiant normalisé.</param>
    /// <returns>Le numéro du compte, ou null.</returns>
    public static int? Find(IReadOnlyList<SeerrAccount> accounts, string jellyfinUserId)
        => accounts.FirstOrDefault(a => a.JellyfinUserId is not null && RelayRules.NormalizeUserId(a.JellyfinUserId) == jellyfinUserId)?.Id;

    /// <summary>
    /// Cookies et jeton à joindre aux envois quand Seerr protège ses formulaires (CSRF) : obtenus par une première
    /// lecture, gardés ensuite.
    /// </summary>
    /// <param name="http">Client HTTP.</param>
    /// <param name="baseUrl">Adresse de Seerr.</param>
    /// <param name="refresh">Redemander (jeton refusé).</param>
    /// <param name="cancel">Annulation.</param>
    /// <returns>Les cookies et le jeton, ou null si Seerr n'en donne pas.</returns>
    public async Task<(string Cookies, string Token)?> Csrf(HttpClient http, string baseUrl, bool refresh, CancellationToken cancel)
    {
        await _gate.WaitAsync(cancel).ConfigureAwait(false);
        try
        {
            if (refresh || _csrfCookies is null)
            {
                using var response = await http.GetAsync(baseUrl + "/api/v1/settings/public", cancel).ConfigureAwait(false);
                var cookies = response.Headers.TryGetValues("Set-Cookie", out var values) ? values : [];
                var pairs = cookies.Select(c => c.Split(';')[0].Trim()).Where(p => p.Contains('=', StringComparison.Ordinal)).ToList();
                var xsrf = pairs.FirstOrDefault(p => p.StartsWith("XSRF-TOKEN=", StringComparison.Ordinal));
                _csrfCookies = string.Join("; ", pairs);
                _xsrfToken = xsrf is null ? string.Empty : Uri.UnescapeDataString(xsrf["XSRF-TOKEN=".Length..]);
            }

            return string.IsNullOrEmpty(_xsrfToken) ? null : (_csrfCookies!, _xsrfToken!);
        }
        finally
        {
            _gate.Release();
        }
    }

    /// <inheritdoc />
    public void Dispose() => _gate.Dispose();
}
