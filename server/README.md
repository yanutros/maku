# Maku Server

Maku Server est un petit relais à installer à côté d'un serveur **Plex**, **Jellyfin** ou **Emby**. Avec Jellyfin 12, le
[plugin Maku](../plugins/jellyfin/) fait la même chose pour Seerr et Tracearr depuis Jellyfin lui-même, sans Docker ni
port en plus. Il permet aux utilisateurs de Maku de faire leurs demandes avec **Seerr** (Overseerr, Jellyseerr), de
suivre leurs statistiques **Tracearr** et de lire les livres, BD et mangas de **Kavita**, sans que ces services soient
exposés sur Internet.

- Seerr et Tracearr restent sur le réseau local. Seul Maku Server est joignable, sur un seul port
  (32700 par défaut).
- Il ne laisse passer que les utilisateurs du serveur. Chaque appel porte le jeton de l'utilisateur
  auprès de Plex, Jellyfin ou Emby, que Maku Server vérifie auprès du serveur (la réponse est gardée
  10 minutes).
- Il ne transmet que l'API de Seerr et de Tracearr (`/api/v1/…`), pas leur site web : leur page de
  connexion et leur administration restent invisibles depuis Internet.
- Seerr et Tracearr voient le compte de l'utilisateur, avec ses droits et ses quotas, comme s'il
  s'y connectait directement.

## Installation (Docker)

```yaml
# docker-compose.yml
services:
  maku-server:
    image: ghcr.io/yanutros/maku-server:latest
    container_name: maku-server
    restart: unless-stopped
    ports:
      - "32700:32700"
    environment:
      # Un seul des trois : l'adresse du serveur, vue depuis le conteneur.
      PLEX_URL: http://192.168.1.10:32400
      # JELLYFIN_URL: http://192.168.1.10:8096
      # EMBY_URL: http://192.168.1.10:8096
      # Ce qu'il faut relayer : un ou plusieurs de ces services.
      SEERR_URL: http://192.168.1.10:5055
      TRACEARR_URL: http://192.168.1.10:3000
      # Kavita : son adresse et la clé d'API de son compte administrateur (voir « Kavita » plus bas).
      # KAVITA_URL: http://192.168.1.10:5000
      # KAVITA_KEY: "clé d'API de l'administrateur de Kavita"
      # Conseillé : une longue phrase au hasard, par laquelle le relais prouve à Maku qui il est (voir plus bas).
      # MAKU_KEY: "collez ici le résultat de : openssl rand -hex 24"
    # Avec Kavita seulement : le dossier où le relais retient le compte Kavita de chacun.
    volumes:
      - maku-data:/data

volumes:
  maku-data:
```

Ou en une ligne :

```sh
docker run -d --name maku-server --restart unless-stopped -p 32700:32700 \
  -e PLEX_URL=http://192.168.1.10:32400 \
  -e SEERR_URL=http://192.168.1.10:5055 \
  -e TRACEARR_URL=http://192.168.1.10:3000 \
  ghcr.io/yanutros/maku-server:latest
```

L'image est faite pour les ordinateurs (amd64) et les serveurs ARM 64 bits (Raspberry Pi, NAS). Elle ne
contient que le programme. Maku Server ne garde rien sur disque, sauf avec Kavita : le compte Kavita de chaque
utilisateur, dans `/data`.

| Variable | Par défaut | Rôle |
|---|---|---|
| `PLEX_URL`, `JELLYFIN_URL` ou `EMBY_URL` | (obligatoire, un seul) | adresse du serveur, vue depuis le conteneur |
| `SEERR_URL` | | adresse de Seerr, vue depuis le conteneur |
| `TRACEARR_URL` | | adresse de Tracearr, vue depuis le conteneur |
| `KAVITA_URL` | | adresse de Kavita, vue depuis le conteneur |
| `KAVITA_KEY` | (obligatoire avec Kavita) | clé d'API du compte administrateur de Kavita |
| `MAKU_DATA` | `/data` | dossier où sont retenus les comptes Kavita (à garder d'une mise à jour à l'autre) |
| `MAKU_LISTEN` | `0.0.0.0:32700` | adresse et port d'écoute |
| `MAKU_KEY` | | phrase secrète (16 caractères au moins) : le relais prouve son identité à Maku |

Le port 32600 est évité : Plex l'utilise pour son service tuner (DVR). Si 32700 est déjà pris sur la machine,
choisissez-en un autre avec `MAKU_LISTEN` ; Maku ne le trouvera alors plus tout seul, et ses adresses se
renseignent dans les Réglages ou l'invitation.

Il faut au moins `SEERR_URL`, `TRACEARR_URL` ou `KAVITA_URL`.

## Kavita : livres, BD et mangas

Kavita ne connaît pas les comptes Plex, Jellyfin ou Emby. Maku Server donne donc à chaque utilisateur **son propre
compte Kavita**, avec sa propre progression de lecture, et l'administrateur choisit qui lit quoi.

1. Dans Kavita, connecté avec le compte administrateur : **Paramètres → Clés d'authentification**, créez une clé.
2. Donnez au relais `KAVITA_URL`, cette clé (`KAVITA_KEY`) et un volume pour `/data`, puis redémarrez-le.
3. Dans Maku, le propriétaire du serveur (Plex, Jellyfin ou Emby) trouve **Livres** dans la catégorie Admin : la liste
   des utilisateurs du serveur, avec une case par bibliothèque Kavita.
   - La première case cochée crée le compte Kavita de l'utilisateur (un nom tiré du sien, un mot de passe tiré au
     hasard que personne n'a besoin de connaître, une clé d'API pour Maku).
   - Tout décocher lui retire l'accès ; son compte et sa progression restent, s'il revient.
   - Personne n'a accès tant que vous ne l'avez pas coché.
4. Chez l'utilisateur, rien à régler : ses bibliothèques apparaissent dans Maku à côté des autres. Le propriétaire du
   serveur, lui, lit avec le compte administrateur de Kavita (celui de la clé), et garde sa progression.

Ce qu'il faut savoir :

- **La clé administrateur ne quitte jamais le relais.** Chaque utilisateur reçoit la clé de son propre compte : Kavita
  lui refuse tout ce que vous ne lui avez pas ouvert, même avec une application modifiée.
- Les appels passent par le relais (`/kavita/api/…`) avec le jeton de l'utilisateur auprès du serveur, comme pour
  Seerr. Les images (couvertures, pages) sont chargées sans ce jeton : le relais ne les laisse passer qu'avec une clé
  Kavita qu'il a vue, depuis moins d'un jour, auprès d'un utilisateur reconnu.
- Les comptes sont retenus dans `/data/kavita.json`, lisible par le seul relais. Gardez ce volume d'une mise à jour à
  l'autre : sans lui, le relais créerait de nouveaux comptes, et chacun repartirait de zéro.
- Avec un dossier de la machine plutôt qu'un volume (`- ./maku-data:/data`), donnez-le à l'utilisateur du relais :
  `sudo chown 65532:65532 ./maku-data`.
- Les comptes créés s'appellent comme l'utilisateur, suivis de la fin de son identifiant (`Camille.123456`) ; ils
  sont visibles dans **Kavita → Paramètres → Utilisateurs**. Leur adresse e-mail, en `@maku.invalid`, ne mène
  nulle part : aucune invitation n'arrive chez personne.
- Le [plugin Maku pour Jellyfin](../plugins/jellyfin/) ne relaie pas encore Kavita : avec Jellyfin, utilisez Maku Server
  pour Kavita.

## Côté Maku

- **Sur le réseau local**, Maku cherche Maku Server tout seul, sur le port 32700 de la machine du serveur.
  Il vérifie que c'est bien celui de son serveur, puis remplit les adresses de Seerr et Tracearr qui
  manquent.
- **Ailleurs**, ce sont les adresses du relais qui remplacent celles de Seerr et de Tracearr, dans
  **Réglages → Services** ou dans l'invitation de l'administrateur :
  - `https://maku.mondomaine.fr/seerr` pour Seerr ;
  - `https://maku.mondomaine.fr/tracearr` pour Tracearr.

Maku ne joint le jeton de l'utilisateur qu'à une adresse dont il a vérifié qu'elle est le relais de son
serveur (`/maku/info`).

## Prouver son identité : `MAKU_KEY`

Le relais dit à Maku de quel serveur il est le relais, mais l'identifiant d'un serveur n'est pas un secret. Sur un
autre réseau (hôtel, bureau), l'adresse locale de votre relais peut être celle d'une autre machine : si elle se
faisait passer pour lui, elle recevrait le jeton de l'utilisateur.

Avec `MAKU_KEY`, le relais a une clé, tirée de cette phrase :

```sh
openssl rand -hex 24
```

- Maku lui envoie un nombre au hasard, que le relais signe avec sa clé.
- À la première rencontre, Maku retient la clé publique du relais pour ce serveur.
- Ensuite, il n'envoie le jeton qu'au relais qui sait signer avec cette clé. Une autre machine, à la même adresse,
  ne reçoit rien : Seerr et Tracearr affichent alors une erreur, au lieu de lui parler.

À savoir :

- **Gardez la même phrase** d'une mise à jour à l'autre : elle redonne la même clé, et rien ne change pour vos
  utilisateurs. La clé n'est pas enregistrée sur disque : elle est tirée de la phrase à chaque démarrage.
- **Si vous la changez** (ou la retirez), Maku ne reconnaît plus le relais. Chaque utilisateur retire puis remet
  l'adresse de Seerr ou de Tracearr dans **Réglages → Services** : Maku retient alors la nouvelle clé.
- **Sans `MAKU_KEY`**, tout fonctionne comme avant : Maku reconnaît le relais à l'identifiant du serveur.
- La première rencontre reste un acte de confiance : faites-la chez vous, ou par une adresse en HTTPS. Et la clé ne
  chiffre rien : depuis Internet, le HTTPS reste nécessaire (voir ci-dessous).

## Depuis Internet : en HTTPS

Maku Server parle en HTTP. Le jeton de l'utilisateur y passe à chaque appel : depuis Internet, placez-le
derrière un proxy inverse HTTPS (Caddy, Nginx Proxy Manager, Traefik, SWAG…) ou un VPN, et n'ouvrez pas
le port 32700 tel quel. Exemple avec Caddy :

```
maku.mondomaine.fr {
    reverse_proxy maku-server:32700
}
```

### Sans ouvrir de port : Cloudflare Tunnel

Un tunnel Cloudflare part de chez vous vers Cloudflare : aucun port à ouvrir sur la box, et Cloudflare fournit le
HTTPS (il faut un nom de domaine géré par Cloudflare, l'offre gratuite suffit).

1. Dans le tableau de bord Cloudflare : **Zero Trust → Networks → Tunnels → Create a tunnel** (type Cloudflared),
   puis copiez le jeton du tunnel.
2. Onglet **Public Hostname** : `maku.mondomaine.fr` → service `http://maku-server:32700`.
3. Lancez `cloudflared` à côté du relais :

```yaml
services:
  maku-server:
    image: ghcr.io/yanutros/maku-server:latest
    restart: unless-stopped
    environment:
      PLEX_URL: http://192.168.1.10:32400
      SEERR_URL: http://192.168.1.10:5055
      TRACEARR_URL: http://192.168.1.10:3000
    # Pas de « ports: » : seul cloudflared y accède, par le réseau Docker.

  cloudflared:
    image: cloudflare/cloudflared:latest
    restart: unless-stopped
    command: tunnel --no-autoupdate run
    environment:
      TUNNEL_TOKEN: <jeton du tunnel>
```

N'ajoutez pas **Cloudflare Access** devant ce nom : Maku ne saurait pas passer sa page de connexion. La protection,
c'est le relais lui-même, qui refuse tout appel sans jeton valide du serveur. Cloudflare terminant le HTTPS, il voit
passer les jetons : si cela vous gêne, préférez votre propre proxy inverse ou un VPN.

## Adresses

| Chemin | Rôle |
|---|---|
| `GET /maku/info` | version, type et identifiant du serveur, services relayés (sans jeton) |
| `/seerr/api/v1/…` | API de Seerr (jeton `X-Maku-Token` obligatoire) |
| `/tracearr/api/v1/…` | API de Tracearr (jeton `X-Maku-Token` obligatoire) |
| `GET /maku/kavita` | clé du compte Kavita de l'utilisateur, s'il a accès (jeton obligatoire) |
| `GET /maku/kavita/admin` | bibliothèques Kavita et accès de chacun (propriétaire du serveur) |
| `PUT /maku/kavita/admin/users/{id}` | bibliothèques ouvertes à un utilisateur ; la première crée son compte |
| `/kavita/api/…` | API de Kavita (jeton obligatoire ; images : clé Kavita vue récemment) |

## Construire

```sh
cd server
cargo test
cargo build --release      # programme : target/release/maku-server
docker build -t maku-server .
```
