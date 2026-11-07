// Mode démo : Maku tourne dans un navigateur, sans serveur Plex, avec une bibliothèque fictive.
// Sert à produire les captures d'écran du README. Ouvrir http://localhost:1420/?demo=accueil
// (vues : accueil, films, categories, fiche, seerr, decouvrir, stats, musique, lecteur).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type {
  Genre,
  Hub,
  Item,
  ItemDetail,
  QueueTrack,
  SeerrDetail,
  SeerrMedia,
  Section,
  TracearrDashboard,
} from "./api";
import { avatar, cover, logo, poster, scene } from "./demo-art";

const MIN = 60_000;

let next = 1000;
function item(p: Partial<Item> & Pick<Item, "kind" | "title">): Item {
  return {
    ratingKey: String(next++),
    subtitle: null,
    year: 2025,
    summary: null,
    poster: null,
    logo: null,
    logoGuid: null,
    thumb: null,
    backdrop: null,
    index: null,
    parentIndex: null,
    showTitle: null,
    showKey: null,
    albumTitle: null,
    albumKey: null,
    durationMs: null,
    viewOffsetMs: null,
    watched: false,
    unwatchedCount: null,
    contentRating: null,
    rating: null,
    genres: [],
    resolution: "1080",
    playable: false,
    ...p,
  };
}

// ---------- Films ----------

const MOVIE_DATA: [string, number, string[], number, string, string][] = [
  ["Les Échos du Nord", 2025, ["Drame", "Aventure"], 8.1, "Au-delà du cercle polaire", "Une cartographe part sur les traces de son père disparu dans les fjords, où chaque écho semble répondre à une question qu'elle n'a pas encore posée."],
  ["Minuit à Lisbonne", 2024, ["Romance", "Comédie"], 7.4, "Une nuit, deux inconnus", "Coincés dans Lisbonne après le dernier tramway, deux voyageurs que tout oppose ont jusqu'à l'aube pour retrouver une lettre égarée."],
  ["La Dernière Orbite", 2026, ["Science-fiction"], 8.6, "Le retour est un aller simple", "Seule à bord d'une station en perdition, une ingénieure doit choisir entre sauver l'équipage endormi et rentrer sur une Terre qui l'a oubliée."],
  ["Le Phare des Brumes", 2023, ["Thriller", "Mystère"], 7.8, "La lumière ment", "Sur une île coupée du monde par la tempête, le gardien d'un phare découvre que les signaux qu'il reçoit viennent d'un navire disparu depuis trente ans."],
  ["Cendres d'Été", 2024, ["Drame"], 7.1, "Le dernier été", "Trois amis d'enfance se retrouvent dans le village de leur jeunesse, le temps d'un été brûlant qui va tout remettre en question."],
  ["Opération Cobalt", 2025, ["Action", "Thriller"], 7.6, "Zéro marge d'erreur", "Une équipe d'élite dispose de quatre heures pour neutraliser un satellite détourné avant qu'il ne plonge l'Europe dans le noir."],
  ["Le Jardin Suspendu", 2022, ["Animation", "Famille"], 8.3, "Il pousse dans les nuages", "Une jeune botaniste découvre un jardin flottant au-dessus de sa ville et l'étrange peuple qui en prend soin."],
  ["Néon Tokyo", 2026, ["Science-fiction", "Action"], 7.9, "La ville ne dort jamais", "Dans un Tokyo saturé de lumière, un livreur de nuit se retrouve en possession d'une mémoire qui ne lui appartient pas."],
  ["Les Marées Hautes", 2023, ["Drame", "Famille"], 7.3, "Ce que la mer rapporte", "Chaque grande marée ramène sur la plage un objet du passé d'une famille de pêcheurs bretons."],
  ["Le Bal des Lucioles", 2025, ["Fantastique", "Romance"], 8.0, "Une nuit par siècle", "Une fois tous les cent ans, la forêt s'illumine et ouvre un passage que seuls les cœurs sincères peuvent franchir."],
  ["Horizon 9", 2024, ["Science-fiction", "Aventure"], 7.7, "Neuf mondes, un seul retour", "Les survivants d'une expédition interstellaire doivent traverser neuf planètes pour retrouver le signal de la Terre."],
  ["Sable Rouge", 2026, ["Western", "Action"], 7.5, "La justice a soif", "Dans un désert sans loi, une shérif solitaire escorte un témoin que tout le monde veut faire taire."],
];

const movies: Item[] = MOVIE_DATA.map(([title, year, genres, rating, tagline, summary], i) =>
  item({
    kind: "movie",
    title,
    year,
    genres,
    rating,
    summary,
    contentRating: i % 3 === 0 ? "Tous publics" : "-12",
    poster: poster(title, tagline),
    backdrop: scene(title),
    thumb: scene(title, 480, 270),
    durationMs: (95 + ((i * 17) % 50)) * MIN,
    playable: true,
    watched: i % 5 === 4,
  }),
);
movies[2].viewOffsetMs = 41 * MIN;
movies[7].viewOffsetMs = 72 * MIN;

// ---------- Séries ----------

const SHOW_DATA: [string, number, string[], number, string, string][] = [
  ["Station Polaris", 2025, ["Science-fiction", "Drame"], 8.7, "Saison 2", "Sur une base de recherche en Antarctique, une équipe isolée capte un signal venu des glaces."],
  ["Les Gardiens du Rail", 2024, ["Policier", "Drame"], 8.2, "Saison 3", "La brigade ferroviaire enquête sur les crimes qui se jouent à bord des trains de nuit européens."],
  ["Atelier 21", 2026, ["Comédie"], 7.6, "Nouvelle série", "Le quotidien déjanté d'un atelier de design au bord de la faillite."],
  ["Brume", 2023, ["Thriller", "Mystère"], 8.4, "Mini-série", "Un village de montagne disparaît chaque matin dans la brume. Un matin, un habitant ne revient pas."],
  ["Code Azur", 2025, ["Action", "Thriller"], 7.9, "Saison 1", "Une hackeuse repentie infiltre le plus grand casino de la Côte d'Azur."],
  ["Les Héritiers", 2024, ["Drame"], 8.0, "Saison 2", "Une famille d'industriels se déchire autour d'un empire bâti sur un secret."],
  ["Mondes Parallèles", 2026, ["SF & Fantastique"], 8.5, "Saison 1", "Un physicien découvre qu'il peut glisser vers d'autres versions de sa propre vie."],
  ["Les Cuisines de Lyon", 2023, ["Documentaire"], 7.8, "Saison 4", "Dans les coulisses des bouchons lyonnais et de leurs chefs."],
];

const shows: Item[] = SHOW_DATA.map(([title, year, genres, rating, tagline, summary], i) =>
  item({
    kind: "show",
    title,
    year,
    genres,
    rating,
    summary,
    poster: poster(title, tagline),
    backdrop: scene(title),
    unwatchedCount: [6, 0, 10, 3, 8, 0, 10, 12][i] || null,
    watched: i === 1 || i === 5,
  }),
);

const EPISODE_TITLES = ["Le Signal", "Sous la glace", "Point de non-retour", "Aurore", "Le Dernier Convoi", "Interférences", "Nuit blanche", "Le Message", "Dérive", "Équinoxe"];

function episodes(show: Item, season: number): Item[] {
  return EPISODE_TITLES.map((title, i) =>
    item({
      kind: "episode",
      title,
      index: i + 1,
      parentIndex: season,
      showTitle: show.title,
      showKey: show.ratingKey,
      subtitle: `${show.title} · S${season}E${i + 1}`,
      poster: show.poster,
      thumb: scene(`${show.title} ${season} ${i}`, 480, 270),
      backdrop: show.backdrop,
      summary: show.summary,
      durationMs: 52 * MIN,
      playable: true,
      watched: i < 3,
      viewOffsetMs: i === 3 ? 21 * MIN : null,
    }),
  );
}

const showEpisodes = new Map(shows.map((s) => [s.ratingKey, episodes(s, 2)]));
const onDeck: Item[] = shows.slice(0, 5).map((s, i) => {
  const ep = showEpisodes.get(s.ratingKey)![3 + (i % 4)];
  return { ...ep, viewOffsetMs: (10 + i * 7) * MIN };
});

// ---------- Musique ----------

const ARTISTS = ["Les Nuits Pastel", "Orbital Café", "Mira Solène", "Kintsugi Club", "Aube Électrique", "Les Voyageurs du Soir"];
const ALBUMS: [string, number][] = [
  ["Lumières de Juin", 0], ["Horizon Bleu", 1], ["Petites Heures", 2], ["Kaléidoscope", 3], ["Rivage", 4], ["Nocturnes", 5], ["Satellites", 1], ["Sous les Pins", 2],
];
const TRACK_NAMES = ["Ouverture", "Rivages", "Sur la route", "Lueur", "Minuit pile", "Comète", "Les Quais", "Retour"];

const artists: Item[] = ARTISTS.map((name) =>
  item({ kind: "artist", title: name, poster: avatar(name), backdrop: scene(name), genres: ["Électro", "Pop"], summary: `${name} est un groupe fictif imaginé pour la démo de Maku.` }),
);
const albums: Item[] = ALBUMS.map(([title, a], i) =>
  item({
    kind: "album",
    title,
    year: 2018 + i,
    subtitle: ARTISTS[a],
    showTitle: ARTISTS[a],
    showKey: artists[a].ratingKey,
    poster: cover(title, ARTISTS[a]),
    genres: ["Électro"],
  }),
);
function tracksOf(album: Item): Item[] {
  return TRACK_NAMES.map((name, i) =>
    item({
      kind: "track",
      title: name,
      index: i + 1,
      subtitle: album.subtitle,
      showTitle: album.subtitle,
      albumTitle: album.title,
      albumKey: album.ratingKey,
      poster: album.poster,
      durationMs: (170 + ((i * 37) % 90)) * 1000,
      playable: true,
    }),
  );
}
const playlists: Item[] = ["Chill du dimanche", "Route des vacances", "Concentration", "Soirée entre amis"].map((title) =>
  item({ kind: "playlist", title, subtitle: "24 titres", poster: cover(title, "Playlist") }),
);

// ---------- Bibliothèques ----------

const SECTIONS: (Section & { items: Item[] })[] = [
  { key: "1", title: "Films", kind: "movie", items: movies },
  { key: "2", title: "Séries TV", kind: "show", items: shows },
  { key: "3", title: "Musique", kind: "artist", items: artists },
];

function hubs(key: string): Hub[] {
  if (key === "1") {
    return [
      { title: "Continuer à regarder", identifier: "movie.inprogress.1", items: movies.filter((m) => m.viewOffsetMs) },
      { title: "Films sortis récemment", identifier: "movie.recentlyreleased.1", items: [...movies].sort((a, b) => (b.year ?? 0) - (a.year ?? 0)) },
      { title: "Récemment ajoutés dans Films", identifier: "movie.recentlyadded.1", items: [...movies].reverse() },
      { title: "Meilleurs films non vus", identifier: "movie.topunwatched.1", items: [...movies].sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0)) },
    ];
  }
  if (key === "2") {
    return [
      { title: "Continuer à regarder", identifier: "tv.inprogress.2", items: onDeck },
      { title: "Récemment ajoutés dans Séries TV", identifier: "tv.recentlyadded.2", items: [...shows].reverse() },
      { title: "Démarrer le visionnage", identifier: "tv.startwatching.2", items: shows.filter((s) => s.unwatchedCount) },
    ];
  }
  return [
    { title: "Musique écoutée récemment", identifier: "music.recent.played.3", items: artists },
    { title: "Récemment ajoutés dans Musique", identifier: "music.recent.added.3", items: albums },
    { title: "Playlists", identifier: "plexr.playlists", items: playlists },
  ];
}

function genres(key: string): Genre[] {
  const pool = key === "2" ? shows : movies;
  const names = [...new Set(pool.flatMap((m) => m.genres))];
  return names.map((title, i) => {
    const list = pool.filter((m) => m.genres.includes(title));
    const filler = pool.filter((m) => !list.includes(m));
    const shown = [...list, ...filler].slice(0, 3);
    return {
      id: String(i),
      title,
      count: list.length * 23 + 7,
      posters: shown.map((m) => m.poster!),
      backdrop: shown[0].backdrop,
    };
  });
}

function findItem(key: string): Item | undefined {
  return [...movies, ...shows, ...artists, ...albums, ...playlists, ...[...showEpisodes.values()].flat()].find(
    (i) => i.ratingKey === key,
  );
}

const CAST = ["Camille Roussel", "Hugo Marchand", "Inès Benali", "Théo Lambert", "Léa Fontaine", "Malik Diallo", "Sarah Meunier", "Jules Garnier"];
const ROLES = ["Elsa", "Capitaine Moreau", "Nora", "Le gardien", "Iris", "Samuel", "Docteure Vidal", "Antoine"];

function detail(key: string): ItemDetail {
  const base = findItem(key) ?? movies[0];
  return {
    ...base,
    tagline: base.kind === "movie" ? MOVIE_DATA.find((m) => m[0] === base.title)?.[4] ?? null : null,
    originalTitle: null,
    backdropHd: base.backdrop,
    studio: "Studio Fictif",
    releaseDate: `${base.year}-03-14`,
    directors: base.kind === "movie" || base.kind === "episode" ? ["Claire Vasseur"] : [],
    writers: base.kind === "movie" || base.kind === "episode" ? ["Antoine Rey", "Claire Vasseur"] : [],
    cast: base.kind === "movie" || base.kind === "show" ? CAST.map((name, i) => ({ id: String(i + 1), name, role: ROLES[i], photo: avatar(name) })) : [],
    video: base.playable ? "4K (HEVC)" : null,
    audioTracks: base.playable
      ? [
          { id: 1, label: "Français (EAC3 5.1)", selected: true },
          { id: 2, label: "Anglais (EAC3 5.1)", selected: false },
        ]
      : [],
    subtitleTracks: base.playable ? [{ id: 3, label: "Français (SRT)", selected: false }] : [],
    versions: [],
  };
}

function children(key: string): Item[] {
  const show = shows.find((s) => s.ratingKey === key);
  if (show) {
    return [1, 2].map((n) =>
      item({ kind: "season", title: `Saison ${n}`, index: n, showTitle: show.title, showKey: show.ratingKey, poster: show.poster, unwatchedCount: n === 2 ? 7 : null, watched: n === 1 }),
    );
  }
  const artist = artists.find((a) => a.ratingKey === key);
  if (artist) return albums.filter((a) => a.showKey === key);
  return [];
}

function tracks(key: string): Item[] {
  const album = albums.find((a) => a.ratingKey === key) ?? albums[0];
  return tracksOf(album);
}

function queue(key: string): QueueTrack[] {
  return tracks(key).map((t) => ({
    ratingKey: t.ratingKey,
    title: t.title,
    artist: t.subtitle,
    artistKey: artists.find((a) => a.title === t.subtitle)?.ratingKey ?? null,
    album: t.albumTitle,
    albumKey: t.albumKey,
    cover: t.poster,
    durationMs: t.durationMs ?? 200_000,
    url: "",
  }));
}

// ---------- Seerr ----------

const SEERR_DATA: [string, "movie" | "tv", number, string][] = [
  ["Les Vents d'Ardoise", "movie", 0, "Deux sœurs reprennent la carrière d'ardoise familiale, menacée de fermeture."],
  ["Archipel", "tv", 2, "Une série d'aventure sur une chaîne d'îles où chaque île obéit à ses propres lois."],
  ["Le Cercle de Craie", "movie", 5, "Un professeur de mathématiques découvre un code caché dans les murs de son lycée."],
  ["Eclipse Totale", "movie", 0, "Pendant les quatre minutes d'une éclipse, le monde entier s'arrête. Presque."],
  ["Les Funambules", "tv", 4, "Une troupe de cirque itinérante traverse l'Europe en quête de son dernier spectacle."],
  ["Mécanique Céleste", "movie", 3, "Un horloger construit une machine capable de prédire les éclipses du cœur."],
  ["Terminus Sud", "tv", 0, "Les destins croisés des voyageurs du dernier train de nuit vers le sud."],
  ["La Cité d'Ambre", "movie", 0, "Une archéologue met au jour une ville figée dans l'ambre depuis dix mille ans."],
  ["Ondes Courtes", "movie", 2, "Un radioamateur capte chaque nuit la voix d'une femme qui annonce les nouvelles du lendemain."],
  ["Les Saisons de Marie", "tv", 5, "Quatre saisons dans la vie d'une vigneronne, de la taille aux vendanges."],
];

const seerr: SeerrMedia[] = SEERR_DATA.map(([title, mediaType, status, overview], i) => ({
  tmdbId: 500 + i,
  mediaType,
  title,
  year: String(2024 + (i % 3)),
  overview,
  poster: poster(title, mediaType === "tv" ? "Série" : "Film"),
  backdrop: scene(title),
  rating: 6.5 + (i % 4) * 0.6,
  status,
}));

function seerrDetail(tmdbId: number): SeerrDetail {
  const m = seerr.find((s) => s.tmdbId === tmdbId) ?? seerr[0];
  return {
    ...m,
    tagline: "Certaines histoires ne s'écrivent qu'une fois.",
    runtime: m.mediaType === "movie" ? 118 : 48,
    certification: "12",
    genres: ["Drame", "Aventure"],
    releaseDate: `${m.year}-10-16`,
    originalTitle: m.title,
    productionStatus: m.mediaType === "movie" ? "Released" : "Returning Series",
    originalLanguage: "fr",
    countries: ["France", "Belgique"],
    studios: ["Studio Fictif", "Production Imaginaire"],
    budget: m.mediaType === "movie" ? 18_000_000 : null,
    revenue: m.mediaType === "movie" ? 54_300_000 : null,
    seasons: m.mediaType === "tv" ? 2 : null,
    episodes: m.mediaType === "tv" ? 16 : null,
    crew: [
      { id: null, name: "Claire Vasseur", role: "Réalisation", photo: null },
      { id: null, name: "Antoine Rey", role: "Scénario", photo: null },
      { id: null, name: "Nadia Ferrand", role: "Production", photo: null },
    ],
    cast: CAST.map((name, i) => ({ id: i + 1, name, role: ROLES[i], photo: avatar(name) })),
    trailer: "https://example.org",
    ratings: { rtCritics: 87, rtAudience: 91, rtUrl: null, imdb: 7.8, imdbUrl: null, tmdb: 7.6 },
    collection: null,
    providers: [
      { name: "Plateforme A", logo: logo("A", 0) },
      { name: "Plateforme B", logo: logo("B", 210) },
      { name: "Plateforme C", logo: logo("C", 280) },
    ],
    keywords: ["amitié", "voyage", "secret de famille", "nature", "mystère", "road trip"],
    plexRatingKey: null,
    tmdbUrl: "https://example.org",
  };
}

// ---------- Tracearr ----------

const dashboard: TracearrDashboard = {
  username: "Camille",
  nowPlaying: [
    {
      title: shows[0].title,
      subtitle: "S2 · É4 — Aurore",
      poster: shows[0].poster,
      progressMs: 23 * MIN,
      durationMs: 52 * MIN,
      paused: false,
      player: "Maku (PC)",
      local: true,
      decision: "directplay",
    },
  ],
  month: { watchMs: 59 * 60 * MIN, listenMs: 26 * 60 * MIN, plays: 312, titles: 148 },
  previous: { watchMs: 48 * 60 * MIN, listenMs: 20 * 60 * MIN, plays: 280, titles: 151 },
  topTitles: [...shows.slice(0, 4), ...movies.slice(0, 6)].map((t, i) => ({
    rank: i + 1,
    title: t.title,
    year: t.year,
    kind: t.kind,
    poster: t.poster,
    ratingKey: t.ratingKey,
    plays: 3 - (i % 3),
    watchMs: (300 - i * 24) * MIN,
    episodes: t.kind === "show" ? 8 - i : null,
  })),
  allTime: {
    daysWatched: 287,
    currentStreak: 6,
    longestStreak: 18,
    totalWatchTimeMs: 41 * 24 * 60 * MIN + 7 * 60 * MIN,
    firstPlay: { mediaTitle: movies[3].title, grandparentTitle: null, startedAt: "2025-01-03T20:12:00Z" },
    biggestBinge: { showTitle: shows[3].title, episodes: 14, day: "2026-02-01" },
    topGenres: [
      { genre: "Drama", plays: 412 },
      { genre: "Sci-Fi & Fantasy", plays: 318 },
      { genre: "Action", plays: 240 },
      { genre: "Science Fiction", plays: 175 },
      { genre: "Adventure", plays: 122 },
    ],
  },
  bingeArt: { ratingKey: shows[3].ratingKey, poster: shows[3].poster, backdrop: shows[3].backdrop },
  firstPlayArt: { ratingKey: movies[3].ratingKey, poster: movies[3].poster, backdrop: movies[3].backdrop },
};

// ---------- Commandes simulées ----------

type Args = Record<string, any>;

function handle(cmd: string, a: Args): unknown {
  switch (cmd) {
    case "get_session":
      return {
        loggedIn: true,
        serverName: "Maison",
        seerrUrl: "https://seerr.exemple.fr",
        mpvPath: null,
        tracearrUrl: "https://tracearr.exemple.fr",
        tracearrConnected: true,
      };
    case "plex_person_items":
      // Démo : chaque personne joue dans une partie du catalogue fictif.
      return [...movies, ...shows].filter((_, i) => i % 3 === Number(a.id) % 3);
    case "plex_sections":
      return SECTIONS.map(({ key, title, kind }) => ({ key, title, kind }));
    case "plex_home":
      return { onDeck, recentlyAdded: [...movies.slice(6), ...shows.slice(0, 4), ...movies.slice(0, 4)] };
    case "plex_section_hubs":
      return hubs(a.key);
    case "plex_section_items": {
      const s = SECTIONS.find((x) => x.key === a.key)!;
      let list = a.kind === 9 ? albums : a.kind === 10 ? albums.flatMap(tracksOf).slice(0, 16) : s.items;
      if (a.genre !== null && a.genre !== undefined) {
        const g = genres(a.key).find((x) => x.id === a.genre);
        list = list.filter((m) => g && m.genres.includes(g.title));
      }
      const all = [...list, ...list.map((m) => ({ ...m, ratingKey: m.ratingKey + "b" }))];
      return { items: a.start ? [] : all, total: all.length };
    }
    case "plex_genres":
      return genres(a.key);
    case "plex_playlists":
      return playlists;
    case "plex_metadata":
      return detail(a.ratingKey);
    case "plex_children":
      return children(a.ratingKey);
    case "plex_tracks":
      return tracks(a.ratingKey);
    case "plex_search":
      return [...movies, ...shows].filter((m) => m.title.toLowerCase().includes(String(a.query).toLowerCase()));
    case "play":
      return { session: "demo", title: findItem(a.ratingKey)?.title ?? "", subtitle: null };
    case "play_music":
      return { session: "demo-music", tracks: queue(a.ratingKey), index: 2 };
    case "seerr_trending":
    case "seerr_slider":
    case "seerr_search":
      return seerr;
    case "seerr_recent_requests":
      return seerr.filter((m) => m.status === 2 || m.status === 3 || m.status === 5);
    case "seerr_watchlist":
      return [...seerr].reverse();
    case "seerr_related":
    case "seerr_person_credits":
      return [...seerr].reverse();
    case "seerr_details":
      return seerrDetail(a.tmdbId);
    case "seerr_tv_seasons":
      return [1, 2].map((n) => ({ seasonNumber: n, name: `Saison ${n}`, episodeCount: 8, status: n === 1 ? 5 : 0 }));
    case "tracearr_dashboard":
      return dashboard;
    case "plugin:window|is_fullscreen":
      return false;
    default:
      return null;
  }
}

/** Installe la simulation puis, une fois l'interface montée, ouvre la vue demandée. */
export async function installDemo(view: string): Promise<() => Promise<void>> {
  mockWindows("main");
  // Captures nettes : pas de fondu ni d'animation en cours au moment de la prise.
  const still = document.createElement("style");
  still.textContent = "*,*::before,*::after{transition:none!important;animation-duration:0s!important;animation-delay:0s!important}";
  document.head.append(still);
  mockIPC((cmd, args) => handle(cmd, (args ?? {}) as Args), { shouldMockEvents: true });

  return async () => {
    const { go } = await import("./nav.svelte");
    const { player, playMusic, play } = await import("./player.svelte");
    const music = () => playMusic(albums[0].ratingKey);
    switch (view) {
      case "films":
        go({ name: "section", key: "1", title: "Films", kind: "movie" });
        break;
      case "categories":
        go({ name: "section", key: "1", title: "Films", kind: "movie", tab: "categories" });
        break;
      case "series":
        go({ name: "section", key: "2", title: "Séries TV", kind: "show" });
        break;
      case "fiche":
        go({ name: "item", ratingKey: movies[0].ratingKey });
        break;
      case "decouvrir":
        go({ name: "discover" });
        break;
      case "seerr":
        go({ name: "seerr", mediaType: "movie", tmdbId: seerr[0].tmdbId });
        break;
      case "stats":
        go({ name: "stats" });
        break;
      case "musique":
        go({ name: "section", key: "3", title: "Musique", kind: "artist" });
        await music();
        break;
      case "ecoute":
        go({ name: "item", ratingKey: albums[0].ratingKey });
        await music();
        player.expanded = true;
        break;
      case "lecteur": {
        // La vraie vidéo est dessinée par mpv : en démo, une scène fixe la remplace.
        await play(movies[2].ratingKey);
        Object.assign(player, { title: movies[2].title, subtitle: "2026", duration: 128 * 60, position: 41 * 60 + 12, bufferedUntil: 55 * 60, buffering: false, paused: true, volume: 80 });
        document.body.style.background = `#000 url("${movies[2].backdrop}") center / cover no-repeat`;
        break;
      }
      default:
        await music();
        break;
    }
    if (view !== "lecteur") player.position = 74;
  };
}
