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
  /** Serveurs utilisés : un seul (plus d'écran de choix), ou les deux. */
  serverKinds: ServerKinds;
  serverName: string | null;
  seerrUrl: string | null;
  mpvPath: string | null;
  tracearrUrl: string | null;
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
  kind: "movie" | "show" | "artist";
}

export interface Item {
  ratingKey: string;
  kind: string;
  title: string;
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

export interface ServerSession {
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
}

export interface Account {
  username: string;
  email: string | null;
  thumb: string | null;
  plexPass: boolean;
  /** Jellyfin : administrateur du serveur (Plex : voir `nav.admin`). */
  admin: boolean | null;
}

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
  resumePlex: () => invoke<void>("resume_plex"),
  resumeJellyfin: () => invoke<void>("resume_jellyfin"),
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
  ) => invoke<Page>("plex_section_items", { key, start, size, kind, genre, sort }),
  genres: (key: string) => invoke<Genre[]>("plex_genres", { key }),
  sectionHubs: (key: string) => invoke<Hub[]>("plex_section_hubs", { key }),
  playlists: (key: string) => invoke<Item[]>("plex_playlists", { key }),
  collectionChoices: (ratingKey: string) => invoke<Item[]>("collection_choices", { ratingKey }),
  addToCollection: (ratingKey: string, collection: string) => invoke<void>("add_to_collection", { ratingKey, collection }),
  createCollection: (ratingKey: string, title: string) => invoke<void>("create_collection", { ratingKey, title }),
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
  setImage: (ratingKey: string, kind: "posters" | "arts", url: string) =>
    invoke<void>("plex_set_image", { ratingKey, kind, url }),
  refreshMetadata: (ratingKey: string) => invoke<void>("plex_refresh_metadata", { ratingKey }),
  analyze: (ratingKey: string) => invoke<void>("plex_analyze", { ratingKey }),
  prefs: () => invoke<Pref[]>("plex_prefs"),
  setPrefs: (changes: Record<string, string>) => invoke<void>("plex_set_prefs", { changes }),
  sessions: () => invoke<ServerSession[]>("plex_sessions"),
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
  seerrSlider: (slider: SeerrSlider, page = 1) => invoke<SeerrMedia[]>("seerr_slider", { slider, page }),
  seerrRecentRequests: () => invoke<SeerrMedia[]>("seerr_recent_requests"),
  seerrWatchlist: () => invoke<SeerrMedia[]>("seerr_watchlist"),
  seerrTvSeasons: (tmdbId: number) => invoke<SeerrSeason[]>("seerr_tv_seasons", { tmdbId }),
  seerrRequest: (mediaType: string, tmdbId: number, seasons: number[] | null) =>
    invoke<void>("seerr_request", { mediaType, tmdbId, seasons }),

  tracearrConfigure: (url: string) => invoke<void>("tracearr_configure", { url }),
  tmdbConfigure: (key: string) => invoke<void>("tmdb_configure", { key }),
  tracearrStartLogin: (open = true) => invoke<{ handle: string; authUrl: string }>("tracearr_start_login", { open }),
  tracearrPollLogin: (handle: string) => invoke<"waiting" | "done" | "expired">("tracearr_poll_login", { handle }),
  tracearrDashboard: () =>
    invoke<TracearrDashboard>("tracearr_dashboard", { timezone: Intl.DateTimeFormat().resolvedOptions().timeZone }),
  tracearrLogout: () => invoke<void>("tracearr_logout"),

  updateCheck: () => invoke<UpdateInfo | null>("update_check"),
  releaseNotes: () => invoke<UpdateInfo>("release_notes"),
  /** Sur Android, renvoie l'APK téléchargé, à confier à `PlexrDevice.installApk`. */
  updateInstall: (info: UpdateInfo) => invoke<string | null>("update_install", { info }),

  relayStart: () => invoke<string>("relay_start"),
  relayStatus: (status: object) => invoke<void>("relay_status", { status: JSON.stringify(status) }),
  relayStop: () => invoke<void>("relay_stop"),
};

/**
 * Attend que l'utilisateur valide la connexion Plex de Tracearr (5 min max). Avec `onAuthUrl`, le
 * lien n'est pas ouvert dans le navigateur : l'appelant le montre autrement (QR code, téléphone).
 */
export async function tracearrLogin(onAuthUrl?: (url: string) => void): Promise<void> {
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
