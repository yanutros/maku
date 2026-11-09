# Politique de confidentialité de Maku

*English below.*

Maku est un client libre (GPL-3.0) pour vos propres serveurs Plex et Jellyfin. Il ne collecte aucune donnée :
pas de compte Maku, pas de statistiques d'utilisation, pas de publicité, aucun serveur appartenant au projet.

## Ce que Maku contacte

Maku ne communique qu'avec les services que vous utilisez :

- **votre serveur Plex ou Jellyfin**, et **plex.tv** pour la connexion à votre compte Plex et les métadonnées de Plex ;
- **Seerr** et **Tracearr**, seulement si vous les configurez, directement ou par **Maku Server**, le relais que
  l'administrateur de votre serveur peut installer à côté de lui (Maku le cherche sur le port 32700 de la machine du
  serveur, et ne lui confie votre jeton qu'après avoir vérifié qu'il est le relais de ce serveur) ;
- **lrclib.net**, pour les paroles des morceaux (titre, artiste, album et durée), sauf si vous désactivez les paroles ;
- **Wikidata** et **Allociné**, pour les notes Allociné d'un film ou d'une série (l'identifiant IMDb du titre, puis
  son identifiant Allociné), sauf si vous masquez Allociné dans Réglages → Interface ;
- **TMDB** (themoviedb.org) et **fanart.tv** pour des logos et bandes-annonces, avec votre propre clé TMDB si vous en
  indiquez une ;
- **GitHub**, pour rechercher les nouvelles versions (pas dans la version du Microsoft Store, mise à jour par le Store) ;
- **YouTube**, quand vous ouvrez une bande-annonce.

## Ce que Maku conserve

Vos jetons de connexion et vos réglages restent sur votre appareil, dans le dossier de l'application. Ils ne sont
envoyés qu'aux services ci-dessus, pour vous y connecter. Désinstaller Maku les supprime.

Sur Android, Maku tient un journal de diagnostic (appareil, lectures, erreurs, mémoire libre), sans adresse de
serveur, adresse IP, jeton ni identifiant. Il reste sur l'appareil : il n'est transmis que si vous le partagez
vous-même depuis Réglages → Lecteur.

## Contact

Questions ou signalements : [ouvrez un ticket sur GitHub](https://github.com/yanutros/maku/issues).

---

# Maku privacy policy

Maku is a free (GPL-3.0) client for your own Plex and Jellyfin servers. It collects no data: no Maku account, no
usage statistics, no advertising, and no servers run by the project.

## What Maku connects to

Maku only talks to the services you use:

- **your Plex or Jellyfin server**, and **plex.tv** for signing in to your Plex account and Plex metadata;
- **Seerr** and **Tracearr**, only if you set them up, directly or through **Maku Server**, the relay your server's
  admin can install next to it (Maku looks for it on port 32700 of the server's machine, and only gives it your token
  after checking it is that server's relay);
- **lrclib.net**, for song lyrics (title, artist, album and duration), unless you turn lyrics off;
- **Wikidata** and **Allociné**, for a movie's or show's Allociné ratings (the title's IMDb ID, then its Allociné
  ID), unless you hide Allociné in Settings → Interface;
- **TMDB** (themoviedb.org) and **fanart.tv** for logos and trailers, with your own TMDB key if you provide one;
- **GitHub**, to check for new versions (not in the Microsoft Store version, which the Store updates);
- **YouTube**, when you open a trailer.

## What Maku stores

Your sign-in tokens and settings stay on your device, in the app's folder. They are only sent to the services
above, to sign you in. Uninstalling Maku removes them.

On Android, Maku keeps a diagnostic log (device, playbacks, errors, free memory), without server addresses, IP
addresses, tokens or IDs. It stays on the device: it is only sent if you share it yourself from Settings → Player.

## Contact

Questions or reports: [open an issue on GitHub](https://github.com/yanutros/maku/issues).
