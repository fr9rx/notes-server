# notes-server

A backend for a course-notes website, organised as **Course → Chapter → Note → images**.

- **Normal users** don't need an account. They can browse everything and upload notes (a title, optional text and 1–10 images).
- **The admin** manages courses and chapters, and is the only one who can edit or delete anything.

When a note is uploaded, the server:

1. checks each image's format from its actual bytes;
2. applies EXIF rotation and strips all metadata, including phone GPS;
3. resizes the image to fit within **1600px** and makes a **300px thumbnail** (pure Rust, SIMD: NEON on the Pi, AVX2 on x86);
4. saves both as JPEG files on disk;
5. records their locations in SQLite;
6. serves them over **HTTPS** with permanent caching.

The code is built on a Windows laptop and deployed to a **Raspberry Pi 5**.

## API

The server returns JSON everywhere. Errors look like `{"error": "..."}`. Admin routes need `Authorization: Bearer <ADMIN_TOKEN>`.

| Method & path | Who | |
|---|---|---|
| `GET /api/courses` | public | List of courses |
| `GET /api/courses/{slug}` | public | The course with its chapters, in order |
| `POST /api/courses` | admin | `{"slug":"math-101","name":"Math 101","description":"..."}` |
| `PATCH /api/courses/{slug}` | admin | Any of `slug`, `name`, `description` |
| `DELETE /api/courses/{slug}` | admin | Deletes the course's chapters, notes and files too |
| `POST /api/courses/{slug}/chapters` | admin | `{"title":"Limits","position":0}`; with no `position`, the chapter goes at the end |
| `GET /api/chapters/{id}?limit=50&offset=0` | public | The chapter plus a page of its notes (oldest first) with their images |
| `PATCH /api/chapters/{id}` | admin | `title`, `position` |
| `DELETE /api/chapters/{id}` | admin | Deletes the chapter's notes and files too |
| `POST /api/chapters/{id}/notes` | **public** | multipart form: `title`, `body?`, `author_name?`, `images` (1–10 files, 10 MB each) |
| `GET /api/notes/{id}` | public | The note plus its images |
| `PATCH /api/notes/{id}` | admin | `title`, `body`, `author_name` (`""` clears it), `chapter_id` (moves the note) |
| `DELETE /api/notes/{id}` | admin | |
| `POST /api/notes/{id}/images` | admin | multipart form: `images`; they're added after the existing images |
| `DELETE /api/notes/{id}/images/{image_id}` | admin | |
| `GET /files/...` | public | The image files (URLs appear in the responses as `url` / `thumb_url`) |
| `GET /api/auth/check` | admin | `204` if the token is valid (used by `notes-admin` to log in) |
| `GET /health` | public | `ok` |

Each image in a response looks like this:

```json
{ "id": "...", "position": 0,
  "url": "https://notes.example.com/files/notes/<note>/<image>.jpg",
  "thumb_url": "https://notes.example.com/files/notes/<note>/<image>_thumb.jpg",
  "width": 1600, "height": 1000, "thumb_width": 300, "thumb_height": 188,
  "size_bytes": 162826, "original_filename": "board.jpg", "created_at": "..." }
```

The upload endpoint is open to anyone, so it has these protections:

| Protection | Response |
|---|---|
| Per-IP rate limit (`UPLOAD_BURST` / `UPLOAD_REFILL_SECS`) | 429 |
| Size limits | 413 |
| Anything that isn't a real image | 415 |
| Less than `MIN_FREE_DISK_MB` of disk space left | 507 |

Errors that come from an image name the file: `image 2 (scan.png): unsupported or corrupt image`.

An HTML upload form only needs this:

```html
<form method="post" enctype="multipart/form-data" action="https://notes.example.com/api/chapters/CHAPTER_ID/notes">
  <input name="title" required> <textarea name="body"></textarea> <input name="author_name">
  <input type="file" name="images" accept="image/*" multiple required>
</form>
```

## Admin TUI (`notes-admin`)

`notes-admin` is a terminal app for managing everything on the server: courses, chapters, notes and images. It runs on your laptop and talks to the server's HTTPS API with the admin token, so you never need to SSH into the Pi to manage content.

```
 notes-admin  https://notes.example.com
╭ Courses ─────────╮╭ Chapters · Math 101 ──╮┏ Notes · Limits ━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
│  Math 101        ││   1. Derivatives (0)  │┃▶ Lecture 1  Sam · 3 img · 2026-09-27       ┃
│  Physics         ││   2. Limits  (1)      │┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
│                  ││                       │╭ Note ──────────────────────────────────────╮
│                  ││                       ││Lecture 1                                   │
│                  ││                       ││by Sam · created 2026-09-27 18:54 · ...     │
│                  ││                       │╭ Images ────────────────────────────────────╮
│                  ││                       ││   1. a.jpg  1600×800  513 KB               │
╰──────────────────╯╰───────────────────────╯╰────────────────────────────────────────────╯
 a upload   e edit   d delete   m move   i add images   o open   ? help
```

To start it:

```powershell
cargo run --release --bin notes-admin                          # asks for the URL and token
# or connect straight away:
$env:NOTES_URL = "https://notes.example.com"
$env:NOTES_ADMIN_TOKEN = "<ADMIN_TOKEN from /etc/notes-server/env>"
cargo run --release --bin notes-admin
# a Pi on your LAN with a self-signed certificate:
cargo run --release --bin notes-admin -- --url https://notes-pi.local --ca-cert .\cert.pem
```

After a release build, the binary is `target\release\notes-admin.exe`; you can copy it anywhere. The token is never taken as a command-line argument, so it doesn't end up in your shell history.

| Key | Action |
|---|---|
| `←/→`, `Tab` | Switch column: Courses → Chapters → Notes → Images |
| `↑/↓`, `j/k`, `PgUp/PgDn` | Move the selection |
| `a` | Add a course, chapter or note to the focused column (a note needs at least one image) |
| `e` / `Enter` | Edit the selected item (a course's slug, name and description; a chapter's title and position; a note's title, author and text) |
| `d` / `Del` | Delete the selected item, after a confirmation |
| `Shift+↑/↓` or `K/J` | Reorder chapters |
| `m` | Move the selected note to another chapter, in any course |
| `i` | Add images to the selected note |
| `o` / `Enter` on an image | Open the image in your browser |
| `r` / `F5` | Reload from the server; `?` shows help; `q` quits |

Image fields accept file paths or a whole folder (every JPEG, PNG, WebP or GIF directly inside it, sorted by name), separated by `;`. You can also drag files onto the terminal window to paste their paths. In a note's text field, `Enter` starts a new line and `Ctrl+S` saves. If the server rejects a form, for example because a slug is already taken, the form stays open with the error so you can fix it.

## Laptop setup (Windows, one time)

```powershell
# Rust is already installed. For cross-compiling to the Pi:
rustup target add aarch64-unknown-linux-gnu
winget install zig.zig
cargo install --locked cargo-zigbuild
```

The builds use zig for one job: it's the C compiler and linker for the little C code in the dependencies (bundled SQLite and `ring`'s crypto). Everything else is Rust, and zig isn't part of the finished binary.

## Develop and test on the laptop

```powershell
cargo test                                         # unit + API + real-HTTPS tests
cargo run --release --example bench_resize         # image pipeline timings
```

To run the server locally, first put a certificate in `certs/`. You can use `mkcert -cert-file certs/cert.pem -key-file certs/key.pem localhost 127.0.0.1`, or the `openssl` command below. Then:

```powershell
$env:ADMIN_TOKEN = "dev-token-at-least-32-characters-long"
cargo run --release           # https://localhost:3443 ; data in .\data, images in .\uploads
```

Try it with curl (use `curl.exe` in PowerShell, since plain `curl` there is an alias for something else):

```powershell
$h = "Authorization: Bearer $env:ADMIN_TOKEN"
curl.exe -k -H $h -H "Content-Type: application/json" -d '{\"slug\":\"math-101\",\"name\":\"Math 101\"}' https://localhost:3443/api/courses
curl.exe -k -H $h -H "Content-Type: application/json" -d '{\"title\":\"Limits\"}' https://localhost:3443/api/courses/math-101/chapters
curl.exe -k -F "title=Lecture 1" -F "images=@C:\path\photo.jpg" https://localhost:3443/api/chapters/<chapter id>/notes
```

## Deploy to the Raspberry Pi 5

The Pi needs **64-bit Raspberry Pi OS** (Bookworm or newer) with SSH enabled. I recommend putting `/var/lib/notes-server` on a USB SSD or an NVMe HAT, because SD cards are slow and wear out.

**First time:**

```powershell
.\deploy\deploy.ps1 -PiHost pi@notes-pi.local -Setup
```

This:

1. cross-compiles the server;
2. creates a `notes` service user;
3. creates `/opt/notes-server`, `/var/lib/notes-server` (the database and uploads) and `/etc/notes-server/env` (the config, with a random `ADMIN_TOKEN` generated for you);
4. installs the systemd unit.

After it finishes:

1. Edit the config on the Pi: `sudo nano /etc/notes-server/env`. Set `PUBLIC_BASE_URL`, and `CORS_ORIGIN` if the website is served from a different domain.
2. Install a certificate (see below).
3. Deploy again (next section).

**Every update after that:**

```powershell
.\deploy\deploy.ps1 -PiHost pi@notes-pi.local             # build, copy, restart, check it's running
.\deploy\deploy.ps1 -PiHost pi@notes-pi.local -WithBench  # also copy bench_resize to ~ on the Pi
```

The build output is `target\aarch64-unknown-linux-gnu\release\notes-server`, one file of about 7 MB. It's linked against glibc 2.36, so it doesn't matter which newer Pi OS release is installed.

On the Pi:

- Logs: `journalctl -u notes-server -f`
- Benchmark: `~/bench_resize` (or `~/bench_resize photo.jpg`)
- Admin token: `sudo grep ADMIN_TOKEN /etc/notes-server/env`

### Certificates

The server reads `/etc/notes-server/certs/cert.pem` (the full chain) and `key.pem`. It re-reads them every 12 hours, so renewals are picked up without a restart.

**Public domain (Let's Encrypt).** Point the domain at your router and forward ports 80 and 443 to the Pi. Then on the Pi:

```bash
sudo apt install certbot
sudo cp /path/to/deploy/certbot-deploy-hook.sh /etc/letsencrypt/renewal-hooks/deploy/notes-server.sh  # setup-pi.sh already does this if certbot is installed
# First certificate: the server can't start without one, so issue it with certbot's own listener.
sudo systemctl stop notes-server
sudo certbot certonly --standalone -d notes.example.com
sudo systemctl start notes-server
# Re-issue once in webroot mode so future renewals go through the running server on port 80
# (ACME_WEBROOT) and need no downtime. This rewrites certbot's renewal config.
sudo certbot certonly --webroot -w /var/lib/notes-server/acme -d notes.example.com --force-renewal
sudo certbot renew --dry-run
```

**LAN only (self-signed):**

```bash
sudo openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 825 \
  -subj "/CN=notes-pi.local" -addext "subjectAltName=DNS:notes-pi.local,IP:192.168.1.50" \
  -keyout /etc/notes-server/certs/key.pem -out /etc/notes-server/certs/cert.pem
sudo chgrp notes /etc/notes-server/certs/*.pem && sudo chmod 640 /etc/notes-server/certs/*.pem
```

Browsers will warn about a self-signed certificate. `mkcert` avoids that on devices where you install its root CA.

### Backups

Everything the server stores is in `/var/lib/notes-server`. To take a consistent backup while the server is running:

```bash
sqlite3 /var/lib/notes-server/notes.db ".backup /backup/notes.db"
rsync -a /var/lib/notes-server/uploads/ /backup/uploads/
```

## Configuration

All settings are environment variables. On the Pi they live in `/etc/notes-server/env`; see `deploy/notes-server.env.example`.

| Variable | Default | |
|---|---|---|
| `ADMIN_TOKEN` | **required** | At least 32 characters |
| `PUBLIC_BASE_URL` | `https://localhost:3443` | Used to build image URLs and the HTTP→HTTPS redirect |
| `BIND_ADDR` | `0.0.0.0:3443` | Use `0.0.0.0:443` on the Pi |
| `TLS_CERT_PATH` / `TLS_KEY_PATH` | `./certs/cert.pem` / `./certs/key.pem` | |
| `HTTP_REDIRECT_ADDR` | unset | e.g. `0.0.0.0:80`: redirects plain HTTP to HTTPS |
| `ACME_WEBROOT` | unset | Answers certbot `--webroot` challenges on the redirect port |
| `DATABASE_URL` | `sqlite://data/notes.db?mode=rwc` | |
| `UPLOAD_DIR` | `./uploads` | |
| `CORS_ORIGIN` | unset (any origin) | Set to the website's origin in production |
| `IMAGE_WORKERS` | `2` | How many images are processed at once, across all requests |
| `MIN_FREE_DISK_MB` | `1024` | Uploads are refused when free space drops below this |
| `UPLOAD_BURST` / `UPLOAD_REFILL_SECS` | `5` / `12` | Per-IP limit on public uploads |
| `RUST_LOG` | `notes_server=info,tower_http=info` | Use `notes_server=debug` to log how long each image took |

## Layout

```
src/imaging.rs   decode → orient → flatten → resize (fast_image_resize) → encode (jpeg-encoder)
src/routes/      HTTP handlers: courses, chapters, notes/uploads
src/db.rs        all SQL (sqlx + SQLite, WAL mode); schema in migrations/
src/storage.rs   files on disk: notes/{note_id}/{image_id}.jpg + _thumb.jpg
src/auth.rs      admin bearer-token check (constant-time)
src/tls.rs       rustls (ring), certificate reload, HTTP→HTTPS redirect + ACME webroot
src/bin/notes-admin/  admin TUI (ratatui): api.rs client, app.rs state/actions, ui.rs rendering
deploy/          build/deploy scripts for Windows → Pi, systemd unit, Pi setup
```

Uploads are ordered so the database and disk can't disagree:

- **Upload**: all images are processed first, then written to disk, then recorded in one database transaction. If the database step fails, the files just written are removed.
- **Delete**: the database rows are removed first, then the files.

The worst possible leftover is an orphaned file, never a note pointing at a missing image.
