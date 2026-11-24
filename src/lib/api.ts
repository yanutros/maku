import { invoke } from "@tauri-apps/api/core";
import { device } from "./device";
import { nav } from "./nav.svelte";
import { t } from "./i18n";

export type ServerKinds = "plex" | "jellyfin" | "both";

export interface Session {
  loggedIn: boolean;
  /** Serveur de médias : Plex ou Jellyfin (Emby compris : même API). */
  serverKind: "plex" | "jellyfin";
  /** Nom affiché du type de serveur. */
  serverBrand: "Plex" | "Jellyfin" | "Emby";
  /** Connexions gardées, pour revenir sans se reconnecter (écran de choix). */
  plexRemembered: boolean;
  jellyfinRemembered: { name: string; url: string; emby: boolean } | null;
  embyRemembered: { name: string; url: string; emby: boolean } | null;
  /** Serveurs utilisés : un seul (plus d'écran de choix), ou les deux. */
  serverKinds: ServerKinds;
  serverName: string | null;
  /** Identité du serveur en cours (type et identifiant) : ce que l'interface garde par bibliothèque y est rangé. */
  serverId?: string | null;
  seerrUrl: string | null;
  mpvPath: string | null;
  tracearrUrl: string | null;
  /** Kavita (livres, BD, mangas) connecté pour ce serveur : son adresse (la clé reste dans la configuration). */
  kavitaUrl?: string | null;
  /** Kavita fourni par Maku Server : un compte par utilisateur, réglé par l'administrateur (rien à saisir). */
  kavitaRelay?: boolean;
  tracearrConnected: boolean;
  /** Une clé TMDB est enregistrée (logos des titres que Plex n'a pas). */
  tmdb: boolean;
  /** Plex Home : connexion automatique au dernier profil, sans écran « Qui regarde ? ». */
  autoSignIn: boolean;
  /** Version d'un magasin (Microsoft Store, Google Play) : ni mise à jour intégrée, ni lien Ko-fi. */
  store: boolean;
}

/** Profil d'un Plex Home. */
export interface HomeUser {
  uuid: string;
  title: string;
  thumb: string | null;
  admin: boolean;
  /** Profil géré (enfant…) : contenu filtré par le serveur. */
  restricted: boolean;
  /** Protégé par un code PIN. */
  protected: boolean;
}

export interface LoginPin {
  pinId: number;
  authUrl: string;
  /** Code à saisir sur plex.tv/link. */
  code: string;
}

export interface ServerSummary {
  name: string;
  machineId: string;
  owned: boolean;
  owner: string | null;
  ownerThumb: string | null;
}

export interface Section {
  key: string;
  title: string;
  /** « book » : une bibliothèque Kavita (livres, BD, mangas), à la clé « kavita:<numéro> ». */
  kind: "movie" | "show" | "artist" | "book";
}

/** Kavita : une bibliothèque, et son type (« manga » : lecture de droite à gauche proposée). */
export interface BookLibrary {
  id: number;
  name: string;
  kind: "manga" | "comic" | "book";
}

export interface BookSeries {
  id: number;
  name: string;
  libraryId: number;
  cover: string;
  pages: number;
  pagesRead: number;
  format: "images" | "epub" | "pdf" | "unknown";
}

export interface BookChapter {
  id: number;
  volumeId: number;
  number: string;
  title: string;
  pages: number;
  pagesRead: number;
  cover: string;
  special: boolean;
}

export interface BookVolume {
  id: number;
  name: string;
  cover: string;
  pages: number;
  pagesRead: number;
  chapters: BookChapter[];
}

export interface BookSeriesDetail {
  series: BookSeries;
  summary: string | null;
  libraryKind: BookLibrary["kind"];
  volumes: BookVolume[];
  chapters: BookChapter[];
  specials: BookChapter[];
  continueChapter: number | null;
}

/** Ce qu'il faut au lecteur pour un chapitre ; `pageUrl` contient « {page} », à remplacer (à partir de 0). */
export interface BookChapterInfo {
  chapterId: number;
  volumeId: number;
  seriesId: number;
  libraryId: number;
  libraryKind: BookLibrary["kind"];
  seriesName: string;
  title: string;
  pages: number;
  format: BookSeries["format"];
  pageUrl: string;
  resumePage: number;
  nextChapter: number;
  prevChapter: number;
  /** Pages en largeur (double page déjà assemblée), affichées seules en lecture double page. */
  wide: boolean[];
}

/** Entrée du sommaire d'un livre EPUB : la section où elle commence, et l'ancre dans cette section. */
export interface BookTocItem {
  title: string;
  page: number;
  part: string;
  children: BookTocItem[];
}

/** Administration › Livres (Kavita par Maku Server). `members` : les utilisateurs de Jellyfin ou d'Emby (Plex : null). */
export interface KavitaAdmin {
  libraries: { id: number; name: string; type: number }[];
  users: { id: string; name: string; username: string; libraries: number[] }[];
  members: { id: string; name: string }[] | null;
}

export interface Item {
  ratingKey: string;
  kind: string;
  title: string;
  /** Collection intelligente (remplie par Plex selon des filtres). */
  smart?: boolean;
  subtitle: string | null;
  year: number | null;
  summary: string | null;
  poster: string | null;
  /** Logo officiel du titre (PNG transparent), quand Plex en a un. */
  logo: string | null;
  /** Sans logo sur le serveur : identifiant Plex pour le chercher en ligne (lib/logos.ts). */
  logoGuid: string | null;
  thumb: string | null;
  backdrop: string | null;
  index: number | null;
  parentIndex: number | null;
  showTitle: string | null;
  showKey: string | null;
  /** Saison d'un épisode. */
  seasonKey: string | null;
  albumTitle: string | null;
  albumKey: string | null;
  durationMs: number | null;
  viewOffsetMs: number | null;
  watched: boolean;
  unwatchedCount: number | null;
  contentRating: string | null;
  rating: number | null;
  /** Note de l'utilisateur, de 0 à 10 (5 étoiles = 10). */
  userRating?: number | null;
  genres: string[];
  resolution: string | null;
  playable: boolean;
  /** Dernière lecture sur le serveur (ms depuis 1970). */
  lastPlayed?: number | null;
}

/** Filtres et tris d'une bibliothèque, comme dans Plex Web. */
export interface SectionFilters {
  /** `boolean` : à cocher (non vus, en cours, HDR…) ; sinon une valeur à choisir (genre, année…). */
  filters: { id: string; title: string; boolean: boolean }[];
  /** `desc` : l'ordre va d'abord du plus grand au plus petit (année, date d'ajout…). */
  sorts: { key: string; title: string; desc: boolean }[];
  /** Filtres avancés (Plex) : leurs champs, et les opérateurs de chaque type (« string », « tag », « date »…). */
  fields: { key: string; title: string; kind: string }[];
  operators: { kind: string; ops: { key: string; title: string }[] }[];
}

/** Collection intelligente : sa bibliothèque et sa recherche Plex (vide pour une collection ordinaire). */
export interface SmartCollection {
  sectionKey: string;
  title: string;
  content: string;
  smart: boolean;
  /** Ordre d'une collection ordinaire : 0 date de sortie, 1 alphabétique, 2 personnalisé. */
  sort: number;
}

export interface FilterValue {
  id: string;
  title: string;
}

export interface Track {
  id: number;
  label: string;
  selected: boolean;
}

export interface ItemDetail extends Item {
  tagline: string | null;
  originalTitle: string | null;
  backdropHd: string | null;
  studio: string | null;
  releaseDate: string | null;
  directors: string[];
  writers: string[];
  cast: { id: string | null; name: string; role: string | null; photo: string | null }[];
  video: string | null;
  audioTracks: Track[];
  subtitleTracks: Track[];
  /** Versions du même titre (4K, 1080p…) ; la première correspond aux champs ci-dessus. */
  versions: Version[];
  /** Notes des sites d'évaluation connues du serveur. */
  siteRatings: SiteRating[];
  /** Identifiant IMDb d'un film ou d'une série (pour sa note Allociné). */
  imdbId: string | null;
}

/** Site d'une note : IMDb, Rotten Tomatoes (presse : tomate ; public : pop-corn), TMDB, note du public de Jellyfin, Allociné. */
export type RatingSource = "imdb" | "rtCritic" | "rtAudience" | "tmdb" | "community" | "allocinePress" | "allocineUsers";

/** Note d'un site, sur 10. */
export interface SiteRating {
  source: RatingSource;
  value: number;
  /** Rotten Tomatoes : tomate fraîche ou pop-corn debout. */
  good: boolean;
}

export interface Version {
  label: string;
  video: string | null;
  audioTracks: Track[];
  subtitleTracks: Track[];
}

export interface Page {
  items: Item[];
  total: number;
}

export interface Genre {
  id: string;
  title: string;
  count: number;
  posters: string[];
  backdrop: string | null;
}

export interface UpdateInfo {
  version: string;
  current: string;
  notes: string;
  date: string | null;
  assetUrl: string;
  assetName: string;
  size: number;
}

export interface Hub {
  title: string;
  /** Identifiant Plex de la rangée, indépendant de la langue (« movie.inprogress.1 »…). */
  identifier: string;
  items: Item[];
}

/** Fichier d'un titre et son format (« Voir les informations »). */
export interface MediaInfo {
  container: string | null;
  codec: string | null;
  bitrateKbps: number | null;
  channels: number | null;
  /** En Hz. */
  samplingRate: number | null;
  bitDepth: number | null;
  sizeBytes: number | null;
  durationMs: number | null;
  /** Chemin sur le serveur (montré à l'administrateur seulement). */
  file: string | null;
  addedAt: number | null;
  playCount: number | null;
  /** Vidéo : piste d'image et cadence, piste son par défaut, nombre de sous-titres. */
  video: string | null;
  frameRate: string | null;
  audio: string | null;
  subtitles: number | null;
}

export interface QueueTrack {
  ratingKey: string;
  title: string;
  artist: string | null;
  artistKey: string | null;
  album: string | null;
  albumKey: string | null;
  cover: string | null;
  durationMs: number;
  /** Fichier audio, lu directement par le lecteur intégré sur mobile. */
  url: string;
}

export interface Home {
  onDeck: Item[];
  recentlyAdded: Item[];
}

export interface SeerrMedia {
  tmdbId: number;
  mediaType: "movie" | "tv";
  title: string;
  year: string | null;
  overview: string | null;
  poster: string | null;
  backdrop: string | null;
  rating: number | null;
  status: number;
}

export interface SeerrPerson {
  /** Identifiant TMDB (casting) : ouvre ses autres films et séries. */
  id: number | null;
  name: string;
  role: string | null;
  photo: string | null;
}

export interface SeerrDetail extends SeerrMedia {
  tagline: string | null;
  runtime: number | null;
  certification: string | null;
  genres: string[];
  releaseDate: string | null;
  originalTitle: string | null;
  productionStatus: string | null;
  originalLanguage: string | null;
  countries: string[];
  studios: string[];
  budget: number | null;
  revenue: number | null;
  seasons: number | null;
  episodes: number | null;
  crew: SeerrPerson[];
  cast: SeerrPerson[];
  trailer: string | null;
  ratings: {
    rtCritics: number | null;
    rtAudience: number | null;
    rtUrl: string | null;
    imdb: number | null;
    imdbUrl: string | null;
    tmdb: number | null;
  };
  collection: string | null;
  providers: { name: string; logo: string }[];
  keywords: string[];
  plexRatingKey: string | null;
  tmdbUrl: string;
}

export type SeerrSlider = "trending" | "popularMovies" | "upcomingMovies" | "popularTv" | "upcomingTv";

export interface SeerrSeason {
  seasonNumber: number;
  name: string;
  episodeCount: number;
  status: number;
}

/** Demande Seerr vue par son administrateur (Admin › Demandes). */
export interface SeerrUser {
  id: number;
  name: string;
  avatar: string | null;
}

export interface SeerrRequest {
  id: number;
  /** 1 en attente, 2 validée, 3 refusée, 4 échec, 5 terminée. */
  status: number;
  /** Le titre demandé ; son `status` est celui du média (5 disponible, 4 partiel…). */
  media: SeerrMedia;
  seasons: number[];
  requestedBy: SeerrUser;
  modifiedBy: SeerrUser | null;
  createdAt: string;
  updatedAt: string;
  is4k: boolean;
  serverId: number | null;
  profileId: number | null;
  profileName: string | null;
  rootFolder: string | null;
  languageProfileId: number | null;
  tags: unknown;
}

/** Titre proposé par l'agent de Plex pour « Corriger l'association ». */
export interface MatchResult {
  guid: string;
  name: string;
  year: number | null;
  /** Ressemblance avec le fichier, sur 100. */
  score: number | null;
  thumb: string | null;
  summary: string | null;
  /** Album : son artiste chez l'agent. */
  artist: string | null;
}

export type RequestFilter = "pending" | "processing" | "available" | "failed" | "all";

export interface RequestCounts {
  pending: number;
  processing: number;
  available: number;
  total: number;
}

/** Serveur Radarr (films) ou Sonarr (séries) déclaré dans Seerr. */
export interface ArrServer {
  id: number;
  name: string;
  is4k: boolean;
  isDefault: boolean;
}

export interface ArrOptions {
  profiles: { id: number; name: string }[];
  rootFolders: { path: string; freeSpace: number | null }[];
  activeProfileId: number | null;
  activeDirectory: string | null;
}

export interface RequestChanges {
  mediaType: "movie" | "tv";
  serverId: number | null;
  profileId: number | null;
  rootFolder: string | null;
  languageProfileId: number | null;
  seasons: number[];
  tags: unknown;
}

export interface TracearrTotals {
  watchMs: number;
  listenMs: number;
  plays: number;
  titles: number;
}

export interface TracearrDashboard {
  username: string | null;
  nowPlaying: {
    title: string;
    subtitle: string | null;
    poster: string | null;
    progressMs: number;
    durationMs: number;
    paused: boolean;
    player: string | null;
    local: boolean;
    decision: string | null;
  }[];
  month: TracearrTotals;
  previous: TracearrTotals;
  topTitles: {
    rank: number;
    title: string;
    year: number | null;
    kind: string;
    poster: string | null;
    ratingKey: string | null;
    plays: number;
    watchMs: number;
    episodes: number | null;
  }[];
  allTime: {
    daysWatched?: number;
    currentStreak?: number;
    longestStreak?: number;
    totalWatchTimeMs?: number;
    firstPlay?: { mediaTitle: string; grandparentTitle: string | null; startedAt: string } | null;
    biggestBinge?: { showTitle: string; episodes: number; day: string } | null;
    topGenres?: { genre: string; plays: number }[];
  };
  bingeArt: TracearrArt | null;
  firstPlayArt: TracearrArt | null;
}

export interface TracearrArt {
  ratingKey: string;
  poster: string | null;
  backdrop: string | null;
}

export interface EditField {
  name: string;
  label: string;
  input: "text" | "long" | "date";
  value: string;
  locked: boolean;
}

export interface EditInfo {
  ratingKey: string;
  kind: string;
  title: string;
  fields: EditField[];
  genres: string[] | null;
  genresLocked: boolean;
}

export interface ImageChoice {
  key: string;
  preview: string;
  selected: boolean;
  provider: string | null;
}

export interface Pref {
  id: string;
  label: string;
  summary: string;
  kind: "bool" | "int" | "double" | "text";
  value: string;
  default: string;
  advanced: boolean;
  group: string;
  options: [string, string][];
}

/** Compte du serveur dans les statistiques. */
export interface StatAccount {
  id: number;
  name: string;
  thumb: string | null;
}

export interface Bandwidth {
  points: { at: number; lan: boolean; bytes: number; accountId: number }[];
  accounts: StatAccount[];
}

/** Lectures d'un compte pendant un intervalle : `kind` 1 film, 4 épisode, 10 titre, 13 photo ; durée en secondes. */
export interface MediaStats {
  entries: { at: number; accountId: number; kind: number; count: number; duration: number }[];
  accounts: StatAccount[];
}

export interface PlayHistory {
  entries: {
    ratingKey: string;
    kind: string;
    title: string;
    grandparentKey: string | null;
    grandparentTitle: string | null;
    poster: string | null;
    grandparentPoster: string | null;
    accountId: number;
    viewedAt: number;
  }[];
  accounts: StatAccount[];
}

export interface ServerSession {
  /** Identifiant de la session (pour l'arrêter). */
  sessionId: string;
  user: string;
  userThumb: string | null;
  title: string;
  subtitle: string | null;
  poster: string | null;
  progressMs: number;
  durationMs: number;
  state: string;
  player: string;
  local: boolean;
  transcode: boolean;
  bandwidthKbps: number | null;
  /** Titre musical : jaquette carrée (pochette). */
  music: boolean;
}

export interface Account {
  username: string;
  email: string | null;
  thumb: string | null;
  plexPass: boolean;
  /** Jellyfin : administrateur du serveur (Plex : voir `nav.admin`). */
  admin: boolean | null;
}

/** Maku Server trouvé sur la machine du serveur : son adresse, ses services, et s'il les a configurés tout seul. */
/** `plugin` : le plugin Maku de Jellyfin (et non Maku Server) ; Seerr s'y utilise sans mot de passe. */
export type RelayFound = { url: string; seerr: boolean; tracearr: boolean; plugin?: boolean; applied: boolean };

export const api = {
  session: () => invoke<Session>("get_session"),
  setMpvPath: (path: string | null) => invoke<void>("set_mpv_path", { path }),

  /** `link` : code court pour plex.tv/link, sans ouvrir de navigateur (téléviseurs). */
  startLogin: (link = false) => invoke<LoginPin>("plex_start_login", { link }),
  pollLogin: (pinId: number) => invoke<boolean>("plex_poll_login", { pinId }),
  jellyfinLogin: (url: string, username: string, password: string, emby = false) =>
    invoke<string>("jellyfin_login", { url, username, password, emby }),
  jellyfinQuickStart: (url: string) => invoke<{ code: string; secret: string }>("jellyfin_quick_start", { url }),
  jellyfinQuickPoll: (url: string, secret: string) => invoke<boolean>("jellyfin_quick_poll", { url, secret }),
  logout: () => invoke<void>("plex_logout"),
  forgetServer: () => invoke<void>("plex_forget_server"),
  /** Oublie seulement le compte Plex : Jellyfin et Emby restent connectés. */
  switchPlexAccount: () => invoke<void>("plex_switch_account"),
  resumePlex: () => invoke<void>("resume_plex"),
  /** Sans `emby` : la dernière des deux connexions gardées (Jellyfin ou Emby). */
  resumeJellyfin: (emby?: boolean) => invoke<void>("resume_jellyfin", { emby }),
  setServerKinds: (kinds: ServerKinds) => invoke<void>("set_server_kinds", { kinds }),
  servers: () => invoke<ServerSummary[]>("plex_servers"),
  selectServer: (machineId: string) => invoke<string>("plex_select_server", { machineId }),

  sections: () => invoke<Section[]>("plex_sections"),
  /**
   * `kind` : type Plex à lister (9 = albums, 10 = titres d'une bibliothèque musicale) ; `genre` : identifiant de
   * genre ; `sort` : ordre Plex, par ex. « addedAt:desc » (par titre sinon).
   */
  sectionItems: (
    key: string,
    start: number,
    size: number,
    kind: number | null = null,
    genre: string | null = null,
    sort: string | null = null,
    filters: [string, string][] | null = null,
  ) => invoke<Page>("plex_section_items", { key, start, size, kind, genre, sort, filters }),
  /** Titres par première lettre (« # », A…Z), avec le même genre et les mêmes filtres que `sectionItems` ; vide avec Jellyfin. */
  firstCharacters: (key: string, genre: string | null = null, filters: [string, string][] | null = null) =>
    invoke<{ letter: string; size: number }[]>("plex_first_characters", { key, genre, filters }),
  /** Filtres et tris d'une bibliothèque de films (`kind` « movie ») ou de séries (« show »). */
  sectionFilters: (key: string, kind: string) => invoke<SectionFilters>("plex_section_filters", { key, kind }),
  filterValues: (key: string, kind: string, filter: string) => invoke<FilterValue[]>("plex_filter_values", { key, kind, filter }),
  /** Nouvelle collection intelligente (administrateur) ; rend son identifiant. */
  createSmartCollection: (key: string, kind: string, title: string, filters: [string, string][], sort: string | null) =>
    invoke<string>("plex_create_smart_collection", { key, kind, title, filters, sort }),
  updateSmartCollection: (ratingKey: string, key: string, kind: string, filters: [string, string][], sort: string | null) =>
    invoke<void>("plex_update_smart_collection", { ratingKey, key, kind, filters, sort }),
  smartCollection: (ratingKey: string) => invoke<SmartCollection>("plex_smart_collection", { ratingKey }),
  setCollectionSort: (ratingKey: string, sort: number) => invoke<void>("plex_set_collection_sort", { ratingKey, sort }),
  /** Ordre personnalisé : `item` juste après `after` (en tête avec `null`). */
  moveInCollection: (ratingKey: string, item: string, after: string | null) =>
    invoke<void>("plex_move_in_collection", { ratingKey, item, after }),
  /** Collection intelligente changée en collection ordinaire ; rend son nouvel identifiant. */
  convertCollection: (ratingKey: string) => invoke<string>("plex_convert_collection", { ratingKey }),
  genres: (key: string) => invoke<Genre[]>("plex_genres", { key }),
  sectionHubs: (key: string) => invoke<Hub[]>("plex_section_hubs", { key }),
  playlists: (key: string) => invoke<Item[]>("plex_playlists", { key }),
  collectionChoices: (ratingKey: string) => invoke<Item[]>("collection_choices", { ratingKey }),
  addToCollection: (ratingKey: string, collection: string) => invoke<void>("add_to_collection", { ratingKey, collection }),
  createCollection: (ratingKey: string, title: string) => invoke<void>("create_collection", { ratingKey, title }),
  /** Plusieurs titres d'un coup dans une collection (administrateur, Plex). */
  addManyToCollection: (keys: string[], collection: string) => invoke<void>("plex_add_many_to_collection", { keys, collection }),
  /** Nouvelle collection ordinaire avec ces titres (administrateur, Plex) ; rend son identifiant. */
  createCollectionWith: (key: string, kind: string, title: string, keys: string[]) =>
    invoke<string>("plex_create_collection_with", { key, kind, title, keys }),
  playlistChoices: () => invoke<Item[]>("playlist_choices"),
  addToPlaylist: (ratingKey: string, playlist: string) => invoke<void>("add_to_playlist", { ratingKey, playlist }),
  createPlaylist: (ratingKey: string, title: string) => invoke<void>("create_playlist", { ratingKey, title }),
  allocineRatings: (imdbId: string, show: boolean) => invoke<SiteRating[]>("allocine_ratings", { imdbId, show }),
  mediaInfo: (ratingKey: string) => invoke<MediaInfo>("media_info", { ratingKey }),
  tracks: (ratingKey: string) => invoke<Item[]>("plex_tracks", { ratingKey }),
  home: () => invoke<Home>("plex_home"),
  metadata: (ratingKey: string) => invoke<ItemDetail>("plex_metadata", { ratingKey }),
  children: (ratingKey: string) => invoke<Item[]>("plex_children", { ratingKey }),
  related: (ratingKey: string) => invoke<Item[]>("plex_related", { ratingKey }),
  rate: (ratingKey: string, rating: number | null) => invoke<void>("plex_rate", { ratingKey, rating }),
  removeFromContinue: (ratingKey: string) => invoke<void>("plex_remove_from_continue", { ratingKey }),
  watchlisted: (guid: string) => invoke<boolean>("plex_watchlisted", { guid }),
  setWatchlist: (guid: string, on: boolean) => invoke<void>("plex_set_watchlist", { guid, on }),
  collections: (key: string) => invoke<Item[]>("plex_collections", { key }),
  collectionItems: (ratingKey: string) => invoke<Item[]>("plex_collection_items", { ratingKey }),
  personItems: (id: string) => invoke<Item[]>("plex_person_items", { id }),
  /** Bande-annonce : un élément du serveur à lire, ou une vidéo YouTube ; `null` si rien trouvé. */
  trailer: (ratingKey: string) => invoke<{ ratingKey: string | null; youtube: string | null } | null>("plex_trailer", { ratingKey }),
  remoteLogo: (guid: string, fallback = true) => invoke<string | null>("plex_remote_logo", { guid, fallback }),
  search: (query: string) => invoke<Item[]>("plex_search", { query }),
  setWatched: (ratingKey: string, watched: boolean) => invoke<void>("plex_set_watched", { ratingKey, watched }),

  isAdmin: () => invoke<boolean>("plex_is_admin"),
  account: () => invoke<Account>("plex_account"),
  /** L'adresse du serveur répond-elle encore ? Sinon Maku en cherche une autre (vrai si elle a changé). */
  reconnect: () => invoke<boolean>("reconnect_server"),
  homeUsers: () => invoke<{ users: HomeUser[]; current: string | null }>("plex_home_users"),
  switchUser: (uuid: string, pin: string | null) => invoke<void>("plex_switch_user", { uuid, pin }),
  setAutoSignIn: (on: boolean) => invoke<void>("set_auto_sign_in", { on }),
  editInfo: (ratingKey: string) => invoke<EditInfo>("plex_edit_info", { ratingKey }),
  edit: (ratingKey: string, changes: { name: string; value: string; locked: boolean }[], genres: string[] | null) =>
    invoke<void>("plex_edit", { ratingKey, changes, genres }),
  images: (ratingKey: string, kind: "posters" | "arts") => invoke<ImageChoice[]>("plex_images", { ratingKey, kind }),
  /** Image de l'ordinateur : son contenu en base64 (`data`) ou son chemin (fichier déposé sur la fenêtre). */
  uploadImage: (ratingKey: string, kind: "posters" | "arts", file: { data?: string; path?: string }) =>
    invoke<void>("plex_upload_image", { ratingKey, kind, data: file.data ?? null, path: file.path ?? null }),
  setImage: (ratingKey: string, kind: "posters" | "arts", url: string) =>
    invoke<void>("plex_set_image", { ratingKey, kind, url }),
  refreshMetadata: (ratingKey: string) => invoke<void>("plex_refresh_metadata", { ratingKey }),
  analyze: (ratingKey: string) => invoke<void>("plex_analyze", { ratingKey }),
  plexMatches: (ratingKey: string, title: string | null, year: number | null, artist: string | null = null) =>
    invoke<MatchResult[]>("plex_matches", { ratingKey, title, year, artist }),
  plexFixMatch: (ratingKey: string, guid: string, name: string, year: number | null) => invoke<void>("plex_fix_match", { ratingKey, guid, name, year }),
  plexUnmatch: (ratingKey: string) => invoke<void>("plex_unmatch", { ratingKey }),
  prefs: () => invoke<Pref[]>("plex_prefs"),
  setPrefs: (changes: Record<string, string>) => invoke<void>("plex_set_prefs", { changes }),
  sessions: () => invoke<ServerSession[]>("plex_sessions"),
  /** Arrête une lecture en cours, avec un message pour celui qui regarde (Plex Pass). */
  stopSession: (sessionId: string, reason: string) => invoke<void>("plex_stop_session", { sessionId, reason }),
  /** Débit du serveur : `timespan` 6 secondes, 4 heures, 3 jours ; depuis `since` (secondes). */
  bandwidth: (timespan: number, since: number) => invoke<Bandwidth>("plex_bandwidth", { timespan, since }),
  /** Lectures par compte et par type : `timespan` 3 jours, 2 semaines, 1 mois. */
  mediaStats: (timespan: number, since: number) => invoke<MediaStats>("plex_media_stats", { timespan, since }),
  history: (since: number) => invoke<PlayHistory>("plex_history", { since }),
  scanSection: (key: string) => invoke<void>("plex_scan_section", { key }),
  emptyTrash: (key: string) => invoke<void>("plex_empty_trash", { key }),

  seerrConfigure: (url: string) => invoke<void>("seerr_configure", { url }),
  seerrConfigureJellyfin: (url: string, username: string, password: string) =>
    invoke<void>("seerr_configure_jellyfin", { url, username, password }),
  seerrSearch: (query: string, page = 1) => invoke<SeerrMedia[]>("seerr_search", { query, page }),
  seerrTrending: (page = 1) => invoke<SeerrMedia[]>("seerr_trending", { page }),
  seerrDetails: (mediaType: string, tmdbId: number) => invoke<SeerrDetail>("seerr_details", { mediaType, tmdbId }),
  seerrRelated: (mediaType: string, tmdbId: number, relation: "recommendations" | "similar") =>
    invoke<SeerrMedia[]>("seerr_related", { mediaType, tmdbId, relation }),
  seerrPersonCredits: (personId: number) => invoke<SeerrMedia[]>("seerr_person_credits", { personId }),
  seerrSlider: (slider: SeerrSlider, page = 1) => invoke<SeerrMedia[]>("seerr_slider", { slider, page }),
  seerrRecentRequests: () => invoke<SeerrMedia[]>("seerr_recent_requests"),
  seerrWatchlist: () => invoke<SeerrMedia[]>("seerr_watchlist"),
  seerrTvSeasons: (tmdbId: number) => invoke<SeerrSeason[]>("seerr_tv_seasons", { tmdbId }),
  seerrRequest: (mediaType: string, tmdbId: number, seasons: number[] | null) =>
    invoke<void>("seerr_request", { mediaType, tmdbId, seasons }),
  seerrCanManage: () => invoke<boolean>("seerr_can_manage"),
  seerrRequests: (filter: RequestFilter, skip = 0, take = 20) => invoke<{ results: SeerrRequest[]; total: number }>("seerr_requests", { filter, skip, take }),
  seerrRequestCounts: () => invoke<RequestCounts>("seerr_request_counts"),
  seerrRequestStatus: (id: number, approve: boolean) => invoke<void>("seerr_request_status", { id, approve }),
  seerrRequestRetry: (id: number) => invoke<void>("seerr_request_retry", { id }),
  seerrRequestDelete: (id: number) => invoke<void>("seerr_request_delete", { id }),
  seerrRequestUpdate: (id: number, changes: RequestChanges) => invoke<void>("seerr_request_update", { id, changes }),
  seerrArrServers: (mediaType: "movie" | "tv") => invoke<ArrServer[]>("seerr_arr_servers", { mediaType }),
  seerrArrOptions: (mediaType: "movie" | "tv", serverId: number) => invoke<ArrOptions>("seerr_arr_options", { mediaType, serverId }),

  tracearrConfigure: (url: string) => invoke<void>("tracearr_configure", { url }),
  /** Relais Maku Server sur la machine du serveur : il remplit les adresses de Seerr et Tracearr qui manquent. */
  relayDiscover: () => invoke<RelayFound | null>("relay_discover"),
  tmdbConfigure: (key: string) => invoke<void>("tmdb_configure", { key }),
  // Kavita : livres, BD et mangas (kavita.rs).
  kavitaConfigure: (url: string, key: string) => invoke<string | null>("kavita_configure", { url, key }),
  kavitaLibraries: () => invoke<BookLibrary[]>("kavita_libraries"),
  kavitaSeries: (libraryId: number, page: number, size: number) => invoke<{ items: BookSeries[]; total: number }>("kavita_series", { libraryId, page, size }),
  kavitaOnDeck: (libraryId: number) => invoke<BookSeries[]>("kavita_on_deck", { libraryId }),
  kavitaSeriesDetail: (seriesId: number) => invoke<BookSeriesDetail>("kavita_series_detail", { seriesId }),
  kavitaChapter: (chapterId: number) => invoke<BookChapterInfo>("kavita_chapter", { chapterId }),
  kavitaProgress: (progress: { chapterId: number; volumeId: number; seriesId: number; libraryId: number; page: number }) =>
    invoke<void>("kavita_progress", { progress }),
  kavitaBookPage: (chapterId: number, page: number) => invoke<string>("kavita_book_page", { chapterId, page }),
  kavitaBookToc: (chapterId: number) => invoke<BookTocItem[]>("kavita_book_toc", { chapterId }),
  kavitaRelay: () => invoke<{ admin: boolean; changed: boolean } | null>("kavita_relay"),
  kavitaAdmin: () => invoke<KavitaAdmin>("kavita_admin"),
  kavitaAdminSet: (id: string, name: string, libraries: number[]) =>
    invoke<{ id: string; name: string; libraries: number[] }>("kavita_admin_set", { id, name, libraries }),
  tracearrStartLogin: (open = true) => invoke<{ handle: string; authUrl: string }>("tracearr_start_login", { open }),
  tracearrPollLogin: (handle: string) => invoke<"waiting" | "done" | "expired">("tracearr_poll_login", { handle }),
  tracearrJellyfinLogin: () => invoke<void>("tracearr_jellyfin_login"),
  tracearrDashboard: () =>
    invoke<TracearrDashboard>("tracearr_dashboard", { timezone: Intl.DateTimeFormat().resolvedOptions().timeZone }),
  tracearrLogout: () => invoke<void>("tracearr_logout"),

  updateCheck: () => invoke<UpdateInfo | null>("update_check"),
  releaseNotes: () => invoke<UpdateInfo>("release_notes"),
  /** Sur Android, renvoie l'APK téléchargé, à confier à `PlexrDevice.installApk`. */
  updateInstall: (info: UpdateInfo) => invoke<string | null>("update_install", { info }),

  relayStart: (login?: "Jellyfin" | "Emby", url?: string) => invoke<string>("relay_start", { login, url }),
  relayStatus: (status: object, tmdb = false) => invoke<void>("relay_status", { status: JSON.stringify(status), tmdb }),
  relayStop: () => invoke<void>("relay_stop"),
};

/**
 * Attend que l'utilisateur valide la connexion Plex de Tracearr (5 min max). Avec `onAuthUrl`, le
 * lien n'est pas ouvert dans le navigateur : l'appelant le montre autrement (QR code, téléphone).
 */
export async function tracearrLogin(onAuthUrl?: (url: string) => void): Promise<void> {
  // Serveur Jellyfin : Quick Connect, validé par Maku lui-même ; ni navigateur ni QR code.
  if (nav.serverKind === "jellyfin") return api.tracearrJellyfinLogin();
  // Sur téléviseur, pas de navigateur : le lien s'affiche en QR code (voir App.svelte).
  const show = onAuthUrl ?? (device.tv ? (url: string) => (nav.authUrl = url) : undefined);
  const { handle, authUrl } = await api.tracearrStartLogin(!show);
  show?.(authUrl);
  try {
    for (let i = 0; i < 150; i++) {
      await new Promise((r) => setTimeout(r, 2000));
      if (show && !onAuthUrl && !nav.authUrl) throw new Error(t("errors.loginCancelled"));
      const state = await api.tracearrPollLogin(handle);
      if (state === "done") return;
      if (state === "expired") break;
    }
    throw new Error(t("errors.loginTimeout"));
  } finally {
    if (!onAuthUrl) nav.authUrl = null;
  }
}

/** « 1 j 3 h », « 4 h 28 », « 54 min ». */
export function formatLongDuration(ms: number): string {
  const min = Math.round(ms / 60000);
  const days = Math.floor(min / 1440);
  const hours = Math.floor((min % 1440) / 60);
  if (days) return t("format.daysHours", { days, hours });
  if (hours) return t("format.hoursMinutes", { hours, minutes: String(min % 60).padStart(2, "0") });
  return t("format.minutes", { minutes: min });
}

/** Libellés des statuts Seerr. */
export function seerrStatusLabel(status: number): string | null {
  switch (status) {
    case 2:
      return t("common.c2.seerrStatus.requested");
    case 3:
      return t("common.c2.seerrStatus.processing");
    case 4:
      return t("common.c2.seerrStatus.partial");
    case 5:
      return t("common.c2.seerrStatus.available");
    default:
      return null;
  }
}

/**
 * Titre d'un épisode, sauf s'il n'apprend rien : certaines séries mal reconnues par Plex n'ont qu'un
 * titre générique (« Épisode 6 », « Episode #1.6 »…) qui répète le code « S1 E6 » déjà affiché.
 */
export function episodeTitle(title: string | null | undefined): string | null {
  const t = title?.trim();
  if (!t || /^(?:[ée]pisod(?:e|io)|folge|aflevering|ep\.?|e)\s*#?\s*\d+(?:[.x]\d+)?$/i.test(t)) return null;
  return t;
}

export function formatDuration(ms: number | null): string {
  if (!ms) return "";
  const min = Math.round(ms / 60000);
  return min >= 60
    ? t("format.hoursMinutes", { hours: Math.floor(min / 60), minutes: String(min % 60).padStart(2, "0") })
    : t("format.minutes", { minutes: min });
}

export const MUSIC_KINDS = ["artist", "album", "track", "playlist"];

export function isMusic(kind: string): boolean {
  return MUSIC_KINDS.includes(kind);
}

/** Durée d'un titre : « 3:07 ». */
export function formatClock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
