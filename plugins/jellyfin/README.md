# Plugin Maku pour Jellyfin

Le plugin Maku s'installe dans **Jellyfin 12**. Il permet aux utilisateurs de Maku de faire leurs demandes avec **Seerr**
et de suivre leurs statistiques **Tracearr** par l'adresse même du serveur Jellyfin, sans Docker ni port en plus, et
sans que Seerr et Tracearr soient exposés sur Internet. C'est l'équivalent, dans Jellyfin, de
[Maku Server](../../server/README.md).

- Seuls les utilisateurs du serveur passent : Jellyfin vérifie leur jeton avant d'appeler le plugin.
- **Seerr** s'utilise avec sa clé d'API, au nom du compte Seerr de chaque utilisateur (ses droits, ses quotas) : pas de
  mot de passe à saisir dans Maku. Le compte est celui importé de Jellyfin (même identifiant), ou celui que
  l'administrateur choisit dans les réglages du plugin.
- **Tracearr** est relayé tel quel (sa session tient dans ses cookies). Dans Maku, « Se connecter avec Jellyfin » ouvre
  la session par Quick Connect, validé par Maku lui-même : l'administrateur autorise la connexion des membres dans
  Tracearr (Réglages → Se connecter, et sur le serveur dans Connexions).
- Seule l'API de Seerr et de Tracearr passe (`/api/v1/…`), jamais leurs pages d'administration.

## Installation

1. Dans Jellyfin : **Tableau de bord → Extensions → Gérer les dépôts**, ajoutez
   `https://github.com/yanutros/maku/releases/download/jellyfin-plugin/manifest.json`.
2. Installez **Maku** depuis le catalogue, puis redémarrez Jellyfin.
3. Dans les réglages du plugin : l'adresse de Seerr (vue depuis le serveur) et sa clé d'API (Seerr : Paramètres →
   Général), l'adresse de Tracearr, et si besoin le compte Seerr de chaque utilisateur.

Maku trouve le plugin tout seul : Réglages → Services affiche « Plugin Maku détecté ».

## Adresses

| Chemin | Rôle |
|---|---|
| `GET /Maku/maku/info` | version, identifiant du serveur, services relayés (sans jeton) : Maku le reconnaît ainsi |
| `/Maku/seerr/api/v1/…` | API de Seerr, au nom du compte Seerr de l'utilisateur |
| `/Maku/tracearr/api/v1/…` | API de Tracearr, cookies de session compris |
| `GET /Maku/admin/seerr-users` | comptes Seerr, pour la page de réglages (administrateur de Jellyfin) |

## Construire

```sh
cd plugins/jellyfin
dotnet test Jellyfin.Plugin.Maku.Tests
dotnet build Jellyfin.Plugin.Maku -c Release   # bin/Release/net10.0 : le .dll, meta.json et l'image
```

Un tag `jellyfin-v1.2.3` publie la version dans le catalogue (workflow `.github/workflows/plugin-jellyfin.yml`).
