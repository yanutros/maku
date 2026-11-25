using System;
using System.Collections.Generic;
using Jellyfin.Plugin.Maku.Configuration;
using MediaBrowser.Common.Configuration;
using MediaBrowser.Common.Plugins;
using MediaBrowser.Model.Plugins;
using MediaBrowser.Model.Serialization;

namespace Jellyfin.Plugin.Maku;

/// <summary>
/// Maku pour Jellyfin : relaie Seerr, Tracearr et Kavita par l'adresse du serveur Jellyfin, pour ses seuls utilisateurs.
/// </summary>
public class Plugin : BasePlugin<PluginConfiguration>, IHasWebPages
{
    /// <summary>Identifiant du plugin (catalogue, réglages).</summary>
    public const string PluginId = "662e5017-18be-4b27-9def-d03d195ba88e";

    /// <summary>
    /// Initializes a new instance of the <see cref="Plugin"/> class.
    /// </summary>
    /// <param name="applicationPaths">Dossiers de Jellyfin.</param>
    /// <param name="xmlSerializer">Lecture et écriture des réglages.</param>
    public Plugin(IApplicationPaths applicationPaths, IXmlSerializer xmlSerializer)
        : base(applicationPaths, xmlSerializer)
    {
        Instance = this;
    }

    /// <inheritdoc />
    public override string Name => "Maku";

    /// <inheritdoc />
    public override Guid Id => Guid.Parse(PluginId);

    /// <inheritdoc />
    public override string Description => "Seerr, Tracearr et Kavita pour les utilisateurs de Maku, sans les exposer sur Internet.";

    /// <summary>Gets l'instance chargée par Jellyfin.</summary>
    public static Plugin? Instance { get; private set; }

    /// <inheritdoc />
    public IEnumerable<PluginPageInfo> GetPages()
    {
        return
        [
            new PluginPageInfo
            {
                Name = Name,
                EmbeddedResourcePath = GetType().Namespace + ".Configuration.configPage.html",
            },
        ];
    }
}
