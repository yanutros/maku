// Jaquettes de l'interface TV : une seule forme de données pour les rangées et les grilles,
// quelle que soit l'origine (Plex, Seerr, catégories).
import { episodeTitle, errorText, formatDuration, type Item, type SeerrMedia } from "./api";
import { go, toast } from "./nav.svelte";
import { play, playMusic } from "./player.svelte";
import { t } from "./i18n";

/** `tile` : grande tuile de catégorie (couleur du genre, éventail d'affiches), comme sur ordinateur. */
export type Shape = "poster" | "square" | "round" | "wide" | "tile";

export interface TvCard {
  key: string;
  image: string | null;
  shape: Shape;
  /** Légende sous la jaquette (grilles). */
  label: string;
  progress: number;
  eyebrow: string;
  title: string;
  /** Logo officiel du titre, affiché à la place du texte quand il existe (voir TitleLogo). */
  logo: string | null;
  logoGuid: string | null;
  genres: string[];
  line: string;
  summary: string | null;
  backdrop: string | null;
  open: () => void;
  /** Titre Plex derrière la jaquette : appui long sur OK ou touche Menu ouvrent son menu (TvMenu). */
  item?: Item;
  /** Titre vu en entier : une petite coche dans le coin de la jaquette, comme dans Plex. */
  watched?: boolean;
  /** Tuile de catégorie : teinte du genre et affiches en éventail. */
  hue?: number;
  posters?: string[];
}

export interface TvRow {
  title: string;
  cards: TvCard[];
}

const MUSIC = new Set(["artist", "album", "track", "playlist"]);

const fail = (e: unknown) => toast(errorText(e), true);

/** `resume` : OK relance la lecture (« Continuer à regarder ») au lieu d'ouvrir la fiche. */
export function fromItem(item: Item, eyebrow: string, resume = false): TvCard {
  const music = MUSIC.has(item.kind);
  const parts: string[] = [];
  if (item.kind === "episode") {
    const title = episodeTitle(item.title);
    const code = { season: item.parentIndex ?? "", episode: item.index ?? "" };
    parts.push(title ? t("tv.episodeLine", { ...code, title }) : t("posterCard.episodeCode", code));
  }
  else if (music && item.subtitle) parts.push(item.subtitle);
  if (item.kind !== "episode" && item.year) parts.push(String(item.year));
  if (item.kind !== "episode" && item.genres.length) parts.push(item.genres.slice(0, 2).join(", "));
  if (item.rating) parts.push(`★ ${item.rating.toFixed(1)}`);
  if (item.viewOffsetMs && item.durationMs) parts.push(t("tv.remaining", { time: formatDuration(item.durationMs - item.viewOffsetMs) }));
  else if (item.durationMs && !music) parts.push(formatDuration(item.durationMs));

  let open: () => void;
  if (item.kind === "track") open = () => playMusic(item.ratingKey).catch(fail);
  else if (resume) open = () => play(item.ratingKey).catch(fail);
  else open = () => go({ name: "item", ratingKey: item.ratingKey });

  return {
    key: item.ratingKey,
    image: item.poster,
    shape: item.kind === "artist" ? "round" : music ? "square" : "poster",
    label: item.kind === "episode" ? (item.showTitle ?? item.title) : item.title,
    progress: item.viewOffsetMs && item.durationMs ? Math.min(100, (item.viewOffsetMs / item.durationMs) * 100) : 0,
    eyebrow,
    title: item.kind === "episode" ? (item.showTitle ?? item.title) : item.title,
    logo: item.logo,
    logoGuid: item.logoGuid,
    genres: item.genres,
    line: parts.join("  ·  "),
    summary: item.summary,
    backdrop: item.backdrop ?? item.thumb,
    open,
    item,
    watched: item.watched && !music,
  };
}

export function fromSeerr(m: SeerrMedia): TvCard {
  return {
    key: `${m.mediaType}${m.tmdbId}`,
    image: m.poster,
    shape: "poster",
    label: m.title,
    progress: 0,
    eyebrow: t("tv.toRequest"),
    title: m.title,
    logo: null,
    logoGuid: null,
    genres: [],
    line: [m.year, m.mediaType === "tv" ? t("tv.show") : t("tv.movie"), m.rating ? `★ ${m.rating.toFixed(1)}` : null]
      .filter(Boolean)
      .join("  ·  "),
    summary: m.overview,
    backdrop: m.backdrop,
    open: () => go({ name: "seerr", mediaType: m.mediaType, tmdbId: m.tmdbId }),
  };
}
