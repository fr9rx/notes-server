# notes-server

A backend for a course-notes website, organised as **Course → Chapter → Note → images**. It runs on an **Arduino UNO Q (4 GB)**, and the board's **LED matrix shows the server's status**.

- **Normal users** don't need an account. They can browse everything and upload notes (a title, optional text and 1–10 images).
- **The admin** manages courses and chapters, and is the only one who can edit or delete anything.

When a note is uploaded, the server:

1. checks each image's format from its actual bytes;
2. applies EXIF rotation and strips all metadata, including phone GPS;
3. resizes the image to fit within **1600px** and makes a **300px thumbnail** (pure Rust, SIMD: NEON on the board, AVX2 on x86);
4. saves both as JPEG files on disk;
5. records their locations in SQLite;
6. serves them over **HTTPS** with permanent caching.

The UNO Q has two processors, and this project uses both:

| Chip | Runs | Code |
|---|---|---|
| Qualcomm QRB2210 (4× Cortex-A53, Debian) | `notes-server` (Rust) | `src/` |
| STM32U585 (Cortex-M33) | `notes-matrix`, native **Zephyr** firmware that drives the 8×13 LED matrix | `mcu/notes-matrix/` |

The two chips are separate, with no shared memory; they're connected by a UART (`/dev/ttyHS1` ↔ the STM32's LPUART1). Arduino's **`arduino-router`** owns that UART on the Linux side and routes **MessagePack-RPC** between its clients. The Zephyr firmware registers methods with the router, and the server calls them through the router's Unix socket. Both ends implement MessagePack-RPC directly, without the Arduino Bridge library.

The router keeps running, so Arduino App Lab and the cloud connector keep working. Everything is built on a Windows laptop and deployed over USB with adb.

## Website

The site is a React app in `frontend/`. It's compiled into the server binary, so the board still gets one file to deploy. The server serves it with ETags, a one-year cache on hashed assets, and precompressed brotli/gzip, so the board never compresses anything at request time. Any path outside `/api` and `/files` returns the app, so deep links work.

The design spec is `frontend/DESIGN.md` ("Phosphor & Paper"). The site grows out of the board's blue 8×13 LED matrix:
- **Home:**
  - a canvas LED field whose clouds drift and follow the cursor, with a ripple from the plate on every real request;
  - a live replica of the board's matrix, driven by `GET /api/stats` using the same drawing code as the firmware;
  - a word-by-word headline reveal;
  - counters with rolling digits;
  - course cards with an LED-halftone cover that reveals the real photo on hover, 3D tilt, a spotlight border and an LED monogram.
- **Course:** the card morphs into the page header, and a dotted "signal" rail lights up as you scroll down the chapters.
- **Chapter:** a masonry grid laid out from the known photo sizes (no layout shift), paging with an LED loader, newest/oldest sorting, and drag-and-drop anywhere on the page.
- **Note:** the cover morphs into a lightbox with swipe, swipe-down to dismiss, pinch, double-tap and ctrl+wheel zoom, keyboard shortcuts, a thumbnail strip, blur-up to the full photo, and download or share.
- **Upload:** the button morphs into a sheet with:
  - a dotted dropzone and camera capture;
  - photos you can reorder;
  - per-photo LED progress;
  - its own state for every server error, including a 429 countdown;
  - a success celebration: the board's upload arrow, a burst of LED particles, and then the new photo flies into the lightbox.
- **Throughout:**
  - light and dark themes, switched with a circular reveal;
  - a full `prefers-reduced-motion` fallback;
  - self-hosted fonts, so it works offline on a LAN.

```powershell
cd frontend
npm ci
npm run dev        # http://localhost:5173, proxies /api and /files to https://localhost:3443 (cargo run)
npm run build      # -> frontend/dist, which the next cargo build embeds
npm test           # LED view parity tests against the firmware
node scripts/shots.mjs https://localhost:3443 shots   # screenshots of every screen (Edge/Chrome)
```

`cargo build` without Node still works. `build.rs` puts a placeholder page in `frontend/dist`, and `deploy\build-unoq.ps1` always builds the real site first.

## API

The server returns JSON everywhere. Errors look like `{"error": "..."}`. Admin routes need `Authorization: Bearer <ADMIN_TOKEN>`.

| Method & path | Who | |
|---|---|---|
| `GET /api/courses` | public | List of courses, each with chapter/note/photo counts and a cover thumbnail |
| `GET /api/courses/{slug}` | public | The course with its chapters, in order |
| `POST /api/courses` | admin | `{"slug":"math-101","name":"Math 101","description":"..."}` |
| `PATCH /api/courses/{slug}` | admin | Any of `slug`, `name`, `description` |
| `DELETE /api/courses/{slug}` | admin | Deletes the course's chapters, notes and files too |
| `POST /api/courses/{slug}/chapters` | admin | `{"title":"Limits","position":0}`; with no `position`, the chapter goes at the end |
| `GET /api/chapters/{id}?limit=50&offset=0&order=asc` | public | The chapter plus a page of its notes with their images; `order=desc` for newest first |
| `PATCH /api/chapters/{id}` | admin | `title`, `position` |
| `DELETE /api/chapters/{id}` | admin | Deletes the chapter's notes and files too |
| `POST /api/chapters/{id}/notes` | **public** | multipart form: `title`, `body?`, `author_name?`, `images` (1–10 files, 10 MB each) |
| `GET /api/notes/{id}` | public | The note plus its images |
| `PATCH /api/notes/{id}` | admin | `title`, `body`, `author_name` (`""` clears it), `chapter_id` (moves the note) |
| `DELETE /api/notes/{id}` | admin | |
| `POST /api/notes/{id}/images` | admin | multipart form: `images`; they're added after the existing images |
| `DELETE /api/notes/{id}/images/{image_id}` | admin | |
| `GET /files/...` | public | The image files (URLs appear in the responses as `url` / `thumb_url`) |
| `GET /api/stats` | public | Totals plus the live LED numbers: requests and uploads per second for the last 13 s, uptime, status |
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

## LED matrix status

| Matrix shows | Meaning |
|---|---|
| A dim dot sweeping along the bottom row | The firmware is running and waiting to hear from the server |
| A spinning comet | The server is starting |
| A bar graph plus a blinking top-right pixel | Running. Each column is one second of the last 13 seconds; the bar height is requests on a log scale; bright tops mean images were uploaded; the pixel is the heartbeat |
| The bar graph with the top row blinking | Warning: the disk is nearly full, so uploads are refused (scrolls `DISK LOW`) |
| A solid X | Error: the database isn't answering (scrolls `DB ERROR`), or the server failed to start (`START FAILED`) |
| A **blinking X** | **Server down**: no status for 5 seconds, so it crashed or hung. This is detected by the STM32 itself |
| A dim dash | The server was stopped cleanly |
| An arrow flying up | An image was just uploaded |
| Scrolling `IP 192.168.x.y` | The board's address, shown at start and whenever it changes (`NO NETWORK` if there's none) |

The firmware registers these methods with `arduino-router`, and the server calls them:

| Method | Kind | Params |
|---|---|---|
| `notes/status` | notification, every second | `[state, requests, uploads]`, where the state is `"B"`/`"O"`/`"W"`/`"E"`/`"D"` |
| `notes/text` | notification | `[text]`, scrolled once |
| `notes/hello` | request | `[]` → `"notes-matrix 3"`; the server logs `LED matrix firmware answered` |

The firmware re-registers every 10 s whenever no status is arriving, so it recovers if the router restarts.

The firmware:

- **Charlieplex driver:** the 104 LEDs sit on PF0–PF10. A TIM17 interrupt every 10 µs lights one LED at a time, giving about a 960 Hz refresh with 8 brightness levels. The pin table is Arduino's (Apache-2.0).
- **UART:** interrupt-driven into a ring buffer.
- **MessagePack:** `mpack.c` is a small hand-written reader and writer. It resynchronises after garbage bytes.
- **RPC:** `rpc.c` handles the requests and notifications.
- **Rendering:** `view.c` draws the views.

`mpack.c`, `rpc.c` and `view.c` are plain C with no Zephyr APIs. `tests/host_test.c` exercises them on the PC, feeding in the exact bytes the server sends:

```
zig cc -std=c11 -Wall -Wextra -Isrc tests/host_test.c src/mpack.c src/rpc.c src/view.c -o host_test && ./host_test
```

It builds against upstream Zephyr v4.4.2 for board `arduino_uno_q`: 29 KB of flash and 7 KB of RAM.

The Zephyr firmware replaces Arduino's sketch loader on the STM32, so Arduino sketches don't run while it's installed. The router and App Lab's other features keep working. See "Going back to Arduino App Lab" below to undo it.

## Admin TUI (`notes-admin`)

`notes-admin` is a terminal app for managing everything on the server: courses, chapters, notes and images. It runs on your laptop and talks to the server's HTTPS API with the admin token, so you never need a shell on the board to manage content.

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
# the UNO Q with its self-signed certificate (see "Useful commands" below for getting cert.pem):
cargo run --release --bin notes-admin -- --url https://<hostname>.local --ca-cert .\unoq-cert.pem
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
# Server cross-compile (Rust is already installed):
rustup target add aarch64-unknown-linux-gnu
winget install zig.zig
cargo install --locked cargo-zigbuild

# Zephyr workspace for the STM32 firmware (~1.5 GB, lives in mcu\, git-ignored):
cd mcu
python -m venv .venv
.venv\Scripts\pip install west
.venv\Scripts\west init -l notes-matrix           # manifest: mcu\notes-matrix\west.yml (Zephyr v4.4.2 + STM32 HAL only)
.venv\Scripts\west update --narrow -o=--depth=1
.venv\Scripts\pip install -r zephyr\scripts\requirements-base.txt
cd zephyr; ..\.venv\Scripts\west sdk install -t arm-zephyr-eabi; cd ..\..
```

You also need:
- **adb**, which comes with Android platform-tools or Arduino App Lab. The deploy script finds either one.
- **Node.js LTS** for the website, for example `winget install OpenJS.NodeJS.LTS`, or the portable zip from nodejs.org on your PATH.

The server build uses zig for one job: it's the C compiler and linker for the little C code in the dependencies (bundled SQLite and `ring`'s crypto). Everything else is Rust, and zig isn't part of the finished binary.

## Develop and test on the laptop

```powershell
cargo test                                         # unit + API + real-HTTPS + TUI tests
cargo run --release --example bench_resize         # image pipeline timings
.\deploy\build-unoq.ps1 -FirmwareOnly              # build just the Zephyr firmware
```

To run the server locally, first put a certificate in `certs/`. You can use `mkcert -cert-file certs/cert.pem -key-file certs/key.pem localhost 127.0.0.1`, or `openssl`. Then:

```powershell
$env:ADMIN_TOKEN = "dev-token-at-least-32-characters-long"
cargo run --release           # https://localhost:3443 ; data in .\data, images in .\uploads
```

On Windows, `MATRIX_ROUTER` can point at a plain file, for example `$env:MATRIX_ROUTER = "matrix.bin"`. The server then appends the MessagePack-RPC stream to that file instead of a socket, which is handy for inspecting it.

## Deploy to the Arduino UNO Q

Plug the board into the laptop over USB-C.

**First time:**

```powershell
.\deploy\deploy-unoq.ps1 -Setup
```

This does the following:

1. Builds the server and the firmware.
2. Runs `deploy/setup-unoq.sh` on the board with sudo. It asks for the board's password in your terminal, and:
   - creates a `notes` service user (the router socket is world-writable, so it needs no extra groups);
   - creates `/opt/notes-server`, `/var/lib/notes-server` (the database and uploads, on the eMMC) and `/etc/notes-server/env` (with a random `ADMIN_TOKEN`);
   - makes a self-signed certificate for `https://<hostname>.local`;
   - installs the systemd unit;
   - adds a sudoers rule so later deploys can restart just this service without a password.
3. Flashes the Zephyr firmware onto the STM32 over the board's own SWD lines, using Arduino's `remoteocd` and OpenOCD on the board.
4. Installs and starts the server.

**Every update after that:**

```powershell
.\deploy\deploy-unoq.ps1                # build, flash firmware, install, restart, check it's running
.\deploy\deploy-unoq.ps1 -NoFirmware    # server only
```

Useful commands:

| Task | Command |
|---|---|
| Watch the logs | `adb shell journalctl -u notes-server -f` |
| See the admin token | `adb shell -t sudo grep ADMIN_TOKEN /etc/notes-server/env` |
| Reach the server over USB, without a network | `adb forward tcp:8443 tcp:443`, then open `https://localhost:8443` |
| Put the board on Wi-Fi | `adb shell -t sudo nmcli dev wifi connect "<SSID>" password "<password>"` (the matrix then scrolls its IP) |
| Copy the self-signed cert for `notes-admin --ca-cert` | `adb shell cat /etc/notes-server/certs/cert.pem > unoq-cert.pem` (only after `adb shell -t sudo chmod 644 /etc/notes-server/certs/cert.pem`; the certificate is public, the key isn't) |

**Performance on the board:** a 3840×2400 photo takes about 0.7 s to decode, resize and encode on one A53 core, and two images are processed in parallel.

### Going back to Arduino App Lab

```powershell
.\deploy\deploy-unoq.ps1 -RestoreArduinoLoader     # puts Arduino's sketch loader back on the STM32
```

Nothing else needs undoing, because the router was never touched. The server keeps running; the matrix just stops showing its status until `notes-matrix` is flashed again.

### Certificates

The server reads `/etc/notes-server/certs/cert.pem` (the full chain) and `key.pem`. It re-reads them every 12 hours, so renewals are picked up without a restart. Setup creates a self-signed certificate. Browsers will warn about it; `notes-admin` can trust it with `--ca-cert`.

**Public domain (Let's Encrypt).** Point the domain at your router and forward ports 80 and 443 to the board. Then on the board:

```bash
sudo apt install certbot
sudo cp /path/to/deploy/certbot-deploy-hook.sh /etc/letsencrypt/renewal-hooks/deploy/notes-server.sh
# First certificate: the server can't start without one, so issue it with certbot's own listener.
sudo systemctl stop notes-server
sudo certbot certonly --standalone -d notes.example.com
sudo systemctl start notes-server
# Re-issue once in webroot mode so future renewals go through the running server on port 80
# (ACME_WEBROOT) and need no downtime. This rewrites certbot's renewal config.
sudo certbot certonly --webroot -w /var/lib/notes-server/acme -d notes.example.com --force-renewal
sudo certbot renew --dry-run
```

Then set `PUBLIC_BASE_URL=https://notes.example.com` in `/etc/notes-server/env`.

### Backups

Everything the server stores is in `/var/lib/notes-server`. To take a consistent backup while the server is running:

```bash
sqlite3 /var/lib/notes-server/notes.db ".backup /backup/notes.db"
rsync -a /var/lib/notes-server/uploads/ /backup/uploads/
```

## Configuration

All settings are environment variables. On the board they live in `/etc/notes-server/env`; see `deploy/notes-server.env.example`.

| Variable | Default | |
|---|---|---|
| `ADMIN_TOKEN` | **required** | At least 32 characters |
| `PUBLIC_BASE_URL` | `https://localhost:3443` | Used to build image URLs and the HTTP→HTTPS redirect |
| `BIND_ADDR` | `0.0.0.0:3443` | `0.0.0.0:443` on the board |
| `TLS_CERT_PATH` / `TLS_KEY_PATH` | `./certs/cert.pem` / `./certs/key.pem` | |
| `HTTP_REDIRECT_ADDR` | unset | e.g. `0.0.0.0:80`: redirects plain HTTP to HTTPS |
| `ACME_WEBROOT` | unset | Answers certbot `--webroot` challenges on the redirect port |
| `DATABASE_URL` | `sqlite://data/notes.db?mode=rwc` | |
| `UPLOAD_DIR` | `./uploads` | |
| `MATRIX_ROUTER` | unset (no matrix) | arduino-router's socket: `/var/run/arduino-router.sock` on the UNO Q |
| `CORS_ORIGIN` | unset (any origin) | Set to the website's origin in production |
| `IMAGE_WORKERS` | `2` | How many images are processed at once, across all requests |
| `MIN_FREE_DISK_MB` | `1024` | Uploads are refused (and the matrix warns) below this |
| `UPLOAD_BURST` / `UPLOAD_REFILL_SECS` | `5` / `12` | Per-IP limit on public uploads |
| `RUST_LOG` | `notes_server=info,tower_http=info` | Use `notes_server=debug` to log how long each image took |

## Layout

```
src/imaging.rs        decode → orient → flatten → resize (fast_image_resize) → encode (jpeg-encoder)
src/matrix.rs         LED matrix link: MessagePack-RPC client for arduino-router, 1 Hz status reporter
src/routes/           HTTP handlers: courses, chapters, notes/uploads
src/db.rs             all SQL (sqlx + SQLite, WAL mode); schema in migrations/
src/storage.rs        files on disk: notes/{note_id}/{image_id}.jpg + _thumb.jpg
src/auth.rs           admin bearer-token check (constant-time)
src/tls.rs            rustls (ring), certificate reload, HTTP→HTTPS redirect + ACME webroot
src/web.rs            serves the embedded React app (ETag, immutable assets, precompressed br/gz, SPA fallback)
src/bin/notes-admin/  admin TUI (ratatui): api.rs client, app.rs state/actions, ui.rs rendering
frontend/             React 19 + motion + Tailwind 4 website; DESIGN.md is the design spec
mcu/notes-matrix/     Zephyr firmware: matrix.c (charlieplex), link.c (UART), mpack.c + rpc.c (router protocol), view.c
deploy/               build/deploy scripts (Windows → UNO Q over adb), systemd unit, board setup
```

Uploads are ordered so the database and disk can't disagree:

- **Upload**: all images are processed first, then written to disk, then recorded in one database transaction. If the database step fails, the files just written are removed.
- **Delete**: the database rows are removed first, then the files.

The worst possible leftover is an orphaned file, never a note pointing at a missing image.
