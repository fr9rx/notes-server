import { useInfiniteQuery, useQuery, type QueryClient } from "@tanstack/react-query";

import { ApiError, api } from "./api";
import type { ChapterDetail, Note } from "./types";

export const PAGE_SIZE = 24;
export type SortOrder = "desc" | "asc";

const retry = (count: number, err: unknown) =>
  !(err instanceof ApiError && err.status === 404) && count < 6;
const retryDelay = (attempt: number) => Math.min(2000 * 2 ** attempt, 30_000);

export function useCourses() {
  return useQuery({
    queryKey: ["courses"],
    queryFn: ({ signal }) => api.courses(signal),
    retry,
    retryDelay,
    // While there are no courses, keep checking so the first one pops in by itself.
    refetchInterval: (q) => (q.state.data && q.state.data.length === 0 ? 15_000 : false),
  });
}

export function useCourse(slug: string, enabled = true) {
  return useQuery({
    queryKey: ["course", slug],
    queryFn: ({ signal }) => api.course(slug, signal),
    enabled: enabled && slug !== "",
    retry,
    retryDelay,
  });
}

export function useChapterNotes(chapterId: string, order: SortOrder) {
  return useInfiniteQuery({
    queryKey: ["chapter", chapterId, order],
    queryFn: ({ pageParam, signal }) => api.chapter(chapterId, pageParam, PAGE_SIZE, order, signal),
    initialPageParam: 0,
    getNextPageParam: (last: ChapterDetail) => {
      const next = last.offset + last.notes.length;
      return next < last.total_notes ? next : undefined;
    },
    retry,
    retryDelay,
  });
}

export function useNote(noteId: string, initial?: Note) {
  return useQuery({
    queryKey: ["note", noteId],
    queryFn: ({ signal }) => api.note(noteId, signal),
    initialData: initial,
    retry,
    retryDelay,
  });
}

/** Find a note already loaded in any chapter page (instant lightbox open). */
export function findCachedNote(client: QueryClient, noteId: string): Note | undefined {
  for (const [, data] of client.getQueriesData<{ pages: ChapterDetail[] }>({ queryKey: ["chapter"] })) {
    for (const page of data?.pages ?? []) {
      const n = page.notes.find((x) => x.id === noteId);
      if (n) return n;
    }
  }
  return client.getQueryData<Note>(["note", noteId]);
}
