import type { ChapterDetail, CourseDetail, CourseSummary, Note, NoteImage, Stats } from "./types";

export class ApiError extends Error {
  readonly status: number;
  /** Seconds, from Retry-After on 429. */
  readonly retryAfter?: number;
  constructor(status: number, message: string, retryAfter?: number) {
    super(message);
    this.status = status;
    this.retryAfter = retryAfter;
  }
}

/**
 * The server builds absolute image URLs from its PUBLIC_BASE_URL, but the site
 * may be opened under another name (an IP, `localhost` through `adb forward`).
 * Keep only the path so images always load from wherever the page came from.
 */
export function localUrl(url: string): string {
  try {
    const u = new URL(url, window.location.origin);
    return u.pathname + u.search;
  } catch {
    return url;
  }
}

function localizeImage(img: NoteImage): NoteImage {
  return { ...img, url: localUrl(img.url), thumb_url: localUrl(img.thumb_url) };
}

export function localizeNote(note: Note): Note {
  return { ...note, images: note.images.map(localizeImage) };
}

async function getJson<T>(path: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(path, { signal, headers: { Accept: "application/json" } });
  if (!res.ok) {
    throw new ApiError(res.status, await errorMessage(res));
  }
  return (await res.json()) as T;
}

async function errorMessage(res: Response): Promise<string> {
  try {
    const body = (await res.json()) as { error?: string };
    if (body.error) return body.error;
  } catch {
    // not JSON
  }
  return res.status === 429 ? "Too many uploads at once — wait a few seconds." : `Server returned ${res.status}`;
}

export const api = {
  courses: async (signal?: AbortSignal) =>
    getJson<CourseSummary[]>("/api/courses", signal),

  course: async (slug: string, signal?: AbortSignal) => {
    const c = await getJson<CourseDetail>(`/api/courses/${encodeURIComponent(slug)}`, signal);
    return {
      ...c,
      chapters: c.chapters.map((ch) => ({
        ...ch,
        cover_thumb_url: ch.cover_thumb_url && localUrl(ch.cover_thumb_url),
      })),
    };
  },

  chapter: async (id: string, offset: number, limit: number, order: "asc" | "desc", signal?: AbortSignal) => {
    const c = await getJson<ChapterDetail>(
      `/api/chapters/${encodeURIComponent(id)}?limit=${limit}&offset=${offset}&order=${order}`,
      signal,
    );
    return { ...c, notes: c.notes.map(localizeNote) };
  },

  note: async (id: string, signal?: AbortSignal) =>
    localizeNote(await getJson<Note>(`/api/notes/${encodeURIComponent(id)}`, signal)),

  stats: async (signal?: AbortSignal) => {
    const res = await fetch("/api/stats", { signal, cache: "no-store" });
    if (!res.ok) throw new ApiError(res.status, await errorMessage(res));
    return (await res.json()) as Stats;
  },
};

export interface UploadInput {
  chapterId: string;
  title: string;
  body: string;
  authorName: string;
  files: File[];
}

/**
 * Uploads a note with XMLHttpRequest (fetch has no upload progress).
 * `onProgress` receives 0..1 for the bytes sent; processing on the server
 * happens after 1.0 and before the promise resolves.
 */
export function uploadNote(
  input: UploadInput,
  onProgress: (fraction: number) => void,
  signal?: AbortSignal,
): Promise<Note> {
  const form = new FormData();
  form.append("title", input.title);
  if (input.body) form.append("body", input.body);
  if (input.authorName) form.append("author_name", input.authorName);
  for (const file of input.files) form.append("images", file, file.name);

  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("POST", `/api/chapters/${encodeURIComponent(input.chapterId)}/notes`);
    xhr.responseType = "json";
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) onProgress(e.loaded / e.total);
    };
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        resolve(localizeNote(xhr.response as Note));
      } else {
        const msg = (xhr.response as { error?: string } | null)?.error;
        const after = Number(xhr.getResponseHeader("retry-after") ?? xhr.getResponseHeader("x-ratelimit-after"));
        reject(
          new ApiError(
            xhr.status,
            msg ?? (xhr.status === 429 ? "Too many uploads at once — wait a few seconds." : `Upload failed (${xhr.status})`),
            Number.isFinite(after) && after > 0 ? after : undefined,
          ),
        );
      }
    };
    xhr.onerror = () => reject(new ApiError(0, "Couldn't reach the server. Check your connection."));
    xhr.onabort = () => reject(new ApiError(0, "Upload cancelled."));
    signal?.addEventListener("abort", () => xhr.abort(), { once: true });
    xhr.send(form);
  });
}
