using System;
using System.Collections.Generic;
using System.Linq;

namespace Jellyfin.Plugin.Maku.Relay;

/// <summary>
/// Règles du relais, sans dépendance à Jellyfin (testées à part) : chemins acceptés, en-têtes transmis, cookies.
/// </summary>
public static class RelayRules
{
    /// <summary>En-têtes jamais transmis : propres à la connexion, refaits par le relais, ou d'authentification.</summary>
    private static readonly HashSet<string> Dropped = new(StringComparer.OrdinalIgnoreCase)
    {
        "connection", "keep-alive", "proxy-authenticate", "proxy-authorization", "proxy-connection", "te", "trailer",
        "transfer-encoding", "upgrade", "host", "content-length", "content-type", "origin", "referer", "forwarded",
        "x-forwarded-for", "x-forwarded-host", "x-forwarded-proto", "x-real-ip",
        // Le jeton Jellyfin de l'utilisateur ne sort pas du serveur.
        "authorization", "x-emby-authorization", "x-emby-token", "x-mediabrowser-token", "x-maku-token",
        // Seerr : la clé d'API et l'utilisateur sont posés par le relais, jamais repris de l'appel.
        "x-api-key", "x-api-user",
    };

    /// <summary>
    /// Chemin à transmettre (« api/v1/… ») : l'API seulement, et rien qui remonte d'un dossier, même encodé.
    /// </summary>
    /// <param name="path">Chemin après « /Maku/seerr/ » ou « /Maku/tracearr/ ».</param>
    /// <returns>Le chemin, ou null s'il est refusé.</returns>
    public static string? ApiPath(string? path)
    {
        var p = (path ?? string.Empty).TrimStart('/');
        var api = p.Equals("api/v1", StringComparison.OrdinalIgnoreCase) || p.StartsWith("api/v1/", StringComparison.OrdinalIgnoreCase);
        var lower = p.ToLowerInvariant();
        var escapes = lower.Contains("..", StringComparison.Ordinal) || lower.Contains("%2e", StringComparison.Ordinal)
            || lower.Contains('\\', StringComparison.Ordinal) || lower.Contains("%5c", StringComparison.Ordinal)
            || lower.Contains("%2f", StringComparison.Ordinal);
        return api && !escapes ? p : null;
    }

    /// <summary>L'en-tête de l'appel peut-il être transmis au service ?</summary>
    /// <param name="name">Nom de l'en-tête.</param>
    /// <param name="keepCookies">Tracearr : sa session tient dans ses cookies. Seerr : la clé d'API suffit.</param>
    /// <returns>Vrai s'il est transmis.</returns>
    public static bool Forwarded(string name, bool keepCookies)
    {
        if (Dropped.Contains(name))
        {
            return false;
        }

        return keepCookies || !name.Equals("cookie", StringComparison.OrdinalIgnoreCase);
    }

    /// <summary>L'en-tête de la réponse peut-il être rendu à Maku ?</summary>
    /// <param name="name">Nom de l'en-tête.</param>
    /// <returns>Vrai s'il est rendu.</returns>
    public static bool Returned(string name)
        => !Dropped.Contains(name) && !name.Equals("set-cookie", StringComparison.OrdinalIgnoreCase);

    /// <summary>Retire l'attribut « Domain=… » d'un « Set-Cookie » : le cookie appartient désormais à l'adresse du serveur.</summary>
    /// <param name="cookie">Valeur de l'en-tête.</param>
    /// <returns>Le cookie sans domaine.</returns>
    public static string StripDomain(string cookie)
        => string.Join(';', cookie.Split(';').Where(part => !part.TrimStart().StartsWith("domain=", StringComparison.OrdinalIgnoreCase)));

    /// <summary>Identifiant Jellyfin sous la forme que Seerr enregistre : 32 caractères hexadécimaux, en minuscules.</summary>
    /// <param name="id">Identifiant, avec ou sans tirets.</param>
    /// <returns>L'identifiant normalisé.</returns>
    public static string NormalizeUserId(string? id)
        => (id ?? string.Empty).Replace("-", string.Empty, StringComparison.Ordinal).Trim().ToLowerInvariant();

    /// <summary>Adresse sans « / » final, ou vide si elle n'est pas en http(s).</summary>
    /// <param name="url">Adresse saisie par l'administrateur.</param>
    /// <returns>L'adresse nettoyée.</returns>
    public static string CleanBase(string? url)
    {
        var u = (url ?? string.Empty).Trim().TrimEnd('/');
        return Uri.TryCreate(u, UriKind.Absolute, out var parsed) && (parsed.Scheme == Uri.UriSchemeHttp || parsed.Scheme == Uri.UriSchemeHttps)
            ? u
            : string.Empty;
    }
}
