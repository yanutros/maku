# Plugin Maku pour Jellyfin

Le plugin Maku s'installe dans **Jellyfin 12**. Il permet aux utilisateurs de Maku de faire leurs demandes avec **Seerr**,
de suivre leurs statistiques **Tracearr** et de lire les livres, BD et mangas de **Kavita** par l'adresse même du serveur
Jellyfin, sans Docker ni port en plus, et sans que ces services soient exposés sur Internet. C'est l'équivalent, dans Jellyfin, de
[Maku Server](../../server/README.md).

- Seuls les utilisateurs du serveur passent : Jellyfin vérifie leur jeton avant d'appeler le plugin.
- **Seerr** s'utilise avec sa clé d'API, au nom du compte Seerr de chaque utilisateur (ses droits, ses quotas) : pas de
  mot de passe à saisir dans Maku. Le compte est celui importé de Jellyfin (même identifiant), ou celui que
  l'administrateur choisit dans les réglages du plugin.
- **Tracearr** est relayé tel quel (sa session tient dans ses cookies). Dans Maku, « Se connecter avec Jellyfin » ouvre
  la session par Quick Connect, validé par Maku lui-même : l'administrateur autorise la connexion des membres dans
  Tracearr (Réglages → Se connecter, et sur le serveur dans Connexions).
- **Kavita** : chaque utilisateur reçoit **son propre compte Kavita**, avec sa propre progression. Le plugin le crée
  avec la clé d'API de l'administrateur de Kavita, qui ne quitte jamais le serveur, et l'administrateur de Jellyfin
  choisit qui lit quoi dans Maku, **Admin → Livres**. Les comptes sont retenus dans les réglages du plugin. Les
  administrateurs de Jellyfin lisent avec le compte administrateur de Kavita.
- Seule l'API de Seerr, de Tracearr et de Kavita passe, jamais leurs pages d'administration.

## Installation

1. Dans Jellyfin : **Tableau de bord → Extensions → Gérer les dépôts**, ajoutez
   `https://github.com/yanutros/maku/releases/download/jellyfin-plugin/manifest.json`.
2. Installez **Maku** depuis le catalogue, puis redémarrez Jellyfin.
3. Dans les réglages du plugin : l'adresse de Seerr (vue depuis le serveur) et sa clé d'API (Seerr : Paramètres →
   Général), l'adresse de Tracearr, l'adresse de Kavita et la clé d'API de son compte administrateur (Kavita :
   Paramètres → Clés d'authentification), et si besoin le compte Seerr de chaque utilisateur.

Maku trouve le plugin tout seul : Réglages → Services affiche « Plugin Maku détecté ».

## Adresses

| Chemin | Rôle |
|---|---|
| `GET /Maku/maku/info` | version, identifiant du serveur, services relayés (sans jeton) : Maku le reconnaît ainsi |
| `/Maku/seerr/api/v1/…` | API de Seerr, au nom du compte Seerr de l'utilisateur |
| `/Maku/tracearr/api/v1/…` | API de Tracearr, cookies de session compris |
| `GET /Maku/admin/seerr-users` | comptes Seerr, pour la page de réglages (administrateur de Jellyfin) |
| `GET /Maku/maku/kavita` | clé du compte Kavita de l'utilisateur, s'il a accès |
| `GET /Maku/maku/kavita/admin` | bibliothèques Kavita, comptes et utilisateurs (administrateur de Jellyfin) |
| `PUT /Maku/maku/kavita/admin/users/{id}` | bibliothèques ouvertes à un utilisateur ; la première crée son compte |
| `/Maku/kavita/api/…` | API de Kavita (jeton Jellyfin ; images : clé Kavita vue depuis moins d'un jour) |

## Construire

```sh
cd plugins/jellyfin
dotnet test Jellyfin.Plugin.Maku.Tests
node --test catalogue.test.mjs                 # le catalogue publié à chaque version
dotnet build Jellyfin.Plugin.Maku -c Release   # bin/Release/net10.0 : le .dll, meta.json et l'image
```

Un tag `jellyfin-v1.2.3` publie la version dans le catalogue (workflow `.github/workflows/plugin-jellyfin.yml`).
