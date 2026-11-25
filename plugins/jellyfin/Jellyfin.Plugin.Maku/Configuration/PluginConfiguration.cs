using System.Collections.Generic;
using MediaBrowser.Model.Plugins;

namespace Jellyfin.Plugin.Maku.Configuration;

/// <summary>
/// Réglages du plugin, remplis par l'administrateur dans le tableau de bord de Jellyfin.
/// </summary>
public class PluginConfiguration : BasePluginConfiguration
{
    /// <summary>Gets or sets l'adresse de Seerr, vue depuis le serveur Jellyfin (vide : pas de Seerr).</summary>
    public string SeerrUrl { get; set; } = string.Empty;

    /// <summary>Gets or sets la clé d'API de Seerr (Paramètres → Général) : chaque utilisateur agit sous son compte Seerr.</summary>
    public string SeerrApiKey { get; set; } = string.Empty;

    /// <summary>Gets or sets l'adresse de Tracearr, vue depuis le serveur Jellyfin (vide : pas de Tracearr).</summary>
    public string TracearrUrl { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets les comptes Seerr choisis à la main pour des utilisateurs Jellyfin. Sans lien ici, l'utilisateur
    /// retrouve le compte Seerr importé de Jellyfin (même identifiant).
    /// </summary>
    public List<UserLink> UserLinks { get; set; } = [];

    /// <summary>Gets or sets l'adresse de Kavita, vue depuis le serveur Jellyfin (vide : pas de Kavita).</summary>
    public string KavitaUrl { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets la clé d'API du compte administrateur de Kavita : le plugin crée avec elle le compte Kavita de chaque
    /// utilisateur. Elle ne quitte jamais le serveur.
    /// </summary>
    public string KavitaApiKey { get; set; } = string.Empty;

    /// <summary>Gets or sets les comptes Kavita créés pour les utilisateurs Jellyfin (Maku › Admin › Livres).</summary>
    public List<KavitaAccount> KavitaAccounts { get; set; } = [];
}

/// <summary>Le compte Kavita d'un utilisateur Jellyfin, et les bibliothèques qui lui sont ouvertes.</summary>
public class KavitaAccount
{
    /// <summary>Gets or sets l'identifiant de l'utilisateur Jellyfin (32 caractères hexadécimaux).</summary>
    public string JellyfinUserId { get; set; } = string.Empty;

    /// <summary>Gets or sets son nom, tel que l'administrateur le voit.</summary>
    public string Name { get; set; } = string.Empty;

    /// <summary>Gets or sets le numéro du compte dans Kavita.</summary>
    public int KavitaId { get; set; }

    /// <summary>Gets or sets le nom du compte dans Kavita.</summary>
    public string Username { get; set; } = string.Empty;

    /// <summary>Gets or sets la clé d'API créée pour Maku sur ce compte.</summary>
    public string Key { get; set; } = string.Empty;

    /// <summary>Gets or sets les bibliothèques ouvertes (vide : plus d'accès, le compte et sa progression restent).</summary>
    public List<int> Libraries { get; set; } = [];
}

/// <summary>Un utilisateur Jellyfin et le compte Seerr sous lequel il fait ses demandes.</summary>
public class UserLink
{
    /// <summary>Gets or sets l'identifiant de l'utilisateur Jellyfin (32 caractères hexadécimaux).</summary>
    public string JellyfinUserId { get; set; } = string.Empty;

    /// <summary>Gets or sets le numéro du compte Seerr.</summary>
    public int SeerrUserId { get; set; }
}
