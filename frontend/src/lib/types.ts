// Mirrors src/models.rs on the server.

export interface Course {
  id: string;
  slug: string;
  name: string;
  description: string;
  created_at: string;
  updated_at: string;
}

export interface CourseSummary extends Course {
  chapter_count: number;
  note_count: number;
  image_count: number;
}

export interface Chapter {
  id: string;
  course_id: string;
  title: string;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface ChapterSummary extends Chapter {
  note_count: number;
  image_count: number;
  cover_thumb_url: string | null;
}

export interface CourseDetail extends Course {
  chapters: ChapterSummary[];
}

export interface NoteImage {
  id: string;
  position: number;
  url: string;
  thumb_url: string;
  width: number;
  height: number;
  thumb_width: number;
  thumb_height: number;
  size_bytes: number;
  original_filename: string | null;
  created_at: string;
}

export interface Note {
  id: string;
  chapter_id: string;
  title: string;
  body: string;
  author_name: string | null;
  created_at: string;
  updated_at: string;
  images: NoteImage[];
}

export interface ChapterDetail extends Chapter {
  notes: Note[];
  total_notes: number;
  limit: number;
  offset: number;
}

export type ServerStatus = "starting" | "ok" | "warning" | "error" | "stopping";

export interface Stats {
  courses: number;
  chapters: number;
  notes: number;
  images: number;
  uptime_secs: number;
  status: ServerStatus;
  /** Requests per second over the last 13 s, oldest first. */
  requests: number[];
  /** Uploaded images per second over the last 13 s, oldest first. */
  uploads: number[];
}
