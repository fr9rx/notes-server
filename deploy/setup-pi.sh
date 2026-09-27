#!/usr/bin/env bash
# One-time setup of notes-server on a Raspberry Pi 5 running 64-bit Raspberry Pi OS.
# Run with sudo from a directory containing this script, notes-server.service,
# notes-server.env.example and certbot-deploy-hook.sh (deploy.ps1 -Setup does this).
# Safe to re-run: never overwrites an existing /etc/notes-server/env.
set -euo pipefail

[[ $EUID -eq 0 ]] || { echo "run with sudo" >&2; exit 1; }
[[ "$(uname -m)" == "aarch64" ]] || {
    echo "needs a 64-bit OS (uname -m is $(uname -m)); flash 64-bit Raspberry Pi OS" >&2
    exit 1
}
HERE="$(cd "$(dirname "$0")" && pwd)"
DATA=/var/lib/notes-server

id notes &>/dev/null || useradd --system --home-dir "$DATA" --shell /usr/sbin/nologin notes

install -d -m 755 /opt/notes-server
install -d -m 750 -o notes -g notes "$DATA" "$DATA/uploads"
install -d -m 755 "$DATA/acme"                     # certbot --webroot target (written by root)
install -d -m 750 -o root -g notes /etc/notes-server /etc/notes-server/certs

if [[ ! -f /etc/notes-server/env ]]; then
    install -m 600 -o root -g root "$HERE/notes-server.env.example" /etc/notes-server/env
    token="$(head -c 48 /dev/urandom | base64 | tr -d '/+=\n' | head -c 48)"
    sed -i "s|^ADMIN_TOKEN=.*|ADMIN_TOKEN=$token|" /etc/notes-server/env
    echo "Created /etc/notes-server/env with a random ADMIN_TOKEN (view it: sudo grep ADMIN_TOKEN /etc/notes-server/env)"
fi

if [[ -d /etc/letsencrypt/renewal-hooks/deploy || -x "$(command -v certbot || true)" ]]; then
    install -d -m 755 /etc/letsencrypt/renewal-hooks/deploy
    install -m 755 "$HERE/certbot-deploy-hook.sh" /etc/letsencrypt/renewal-hooks/deploy/notes-server.sh
fi

install -m 644 "$HERE/notes-server.service" /etc/systemd/system/notes-server.service
systemctl daemon-reload
systemctl enable notes-server >/dev/null

cat <<EOF

Setup done. Next:
  1. Edit /etc/notes-server/env (PUBLIC_BASE_URL, CORS_ORIGIN, ...):  sudo nano /etc/notes-server/env
  2. Put a certificate at /etc/notes-server/certs/{cert,key}.pem (see README "Certificates").
  3. Deploy the binary from the laptop: .\\deploy\\deploy.ps1 -PiHost <user>@<pi>
Tip: put $DATA on a USB SSD / NVMe drive rather than the SD card.
EOF
