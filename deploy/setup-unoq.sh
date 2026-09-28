#!/usr/bin/env bash
# One-time setup of notes-server on an Arduino UNO Q (Debian on the QRB2210).
# Run with sudo from a directory containing this script, notes-server.service,
# notes-server.env.example and certbot-deploy-hook.sh (deploy-unoq.ps1 -Setup
# does all of this). Safe to re-run: never overwrites the env file or certs.
set -euo pipefail

[[ $EUID -eq 0 ]] || { echo "run with sudo" >&2; exit 1; }
[[ "$(uname -m)" == "aarch64" ]] || { echo "expected an aarch64 board" >&2; exit 1; }
HERE="$(cd "$(dirname "$0")" && pwd)"
DATA=/var/lib/notes-server
DEPLOY_USER="${SUDO_USER:-arduino}"
HOST="$(hostname)"

# 1. Service user that owns the data. It reaches the LED matrix through
#    arduino-router's socket, which is world-writable, so no extra groups.
id notes &>/dev/null || useradd --system --home-dir "$DATA" --shell /usr/sbin/nologin notes

# 2. Directories. /opt/notes-server belongs to the deploy user so updates need no sudo.
install -d -m 755 -o "$DEPLOY_USER" -g "$DEPLOY_USER" /opt/notes-server
install -d -m 750 -o notes -g notes "$DATA" "$DATA/uploads"
install -d -m 755 "$DATA/acme"
install -d -m 750 -o root -g notes /etc/notes-server /etc/notes-server/certs

# 3. Config with a random admin token.
if [[ ! -f /etc/notes-server/env ]]; then
    install -m 600 -o root -g root "$HERE/notes-server.env.example" /etc/notes-server/env
    token="$(head -c 48 /dev/urandom | base64 | tr -d '/+=\n' | head -c 48)"
    sed -i "s|^ADMIN_TOKEN=.*|ADMIN_TOKEN=$token|; s|^PUBLIC_BASE_URL=.*|PUBLIC_BASE_URL=https://$HOST.local|" /etc/notes-server/env
    echo "Created /etc/notes-server/env (admin token: sudo grep ADMIN_TOKEN /etc/notes-server/env)"
fi

# 4. A self-signed certificate so HTTPS works on the LAN straight away.
#    Replace with a real one for a public domain (README "Certificates").
if [[ ! -f /etc/notes-server/certs/cert.pem ]]; then
    san="DNS:$HOST.local,DNS:$HOST,DNS:localhost,IP:127.0.0.1"
    for ip in $(hostname -I 2>/dev/null); do
        [[ "$ip" == *.* && "$ip" != 172.* ]] && san="$san,IP:$ip"
    done
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 825 \
        -subj "/CN=$HOST.local" -addext "subjectAltName=$san" \
        -keyout /etc/notes-server/certs/key.pem -out /etc/notes-server/certs/cert.pem 2>/dev/null
    chown root:notes /etc/notes-server/certs/*.pem
    chmod 640 /etc/notes-server/certs/*.pem
    echo "Created a self-signed certificate for $san"
fi

if command -v certbot >/dev/null 2>&1; then
    install -d -m 755 /etc/letsencrypt/renewal-hooks/deploy
    install -m 755 "$HERE/certbot-deploy-hook.sh" /etc/letsencrypt/renewal-hooks/deploy/notes-server.sh
fi

# 5. Let the deploy user restart the service without a password (and nothing else).
cat > /etc/sudoers.d/60-notes-server <<EOF
$DEPLOY_USER ALL=(root) NOPASSWD: /usr/bin/systemctl restart notes-server, /usr/bin/systemctl start notes-server, /usr/bin/systemctl stop notes-server
EOF
chmod 440 /etc/sudoers.d/60-notes-server
visudo -cf /etc/sudoers.d/60-notes-server >/dev/null

# 6. systemd unit.
install -m 644 "$HERE/notes-server.service" /etc/systemd/system/notes-server.service
systemctl daemon-reload
systemctl enable notes-server >/dev/null
[[ -x /opt/notes-server/notes-server ]] && systemctl restart notes-server

cat <<EOF

Setup done.
  Config:       /etc/notes-server/env      (sudo nano /etc/notes-server/env)
  Data:         $DATA  (on the eMMC)
  HTTPS:        https://$HOST.local once the board is on your Wi-Fi/LAN
  LED matrix:   via arduino-router (left running; App Lab keeps working)
EOF
