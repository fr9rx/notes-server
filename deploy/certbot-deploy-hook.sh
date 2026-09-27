#!/usr/bin/env bash
# certbot runs this after every successful issue/renewal (installed to
# /etc/letsencrypt/renewal-hooks/deploy/). It copies the new certificate
# where the unprivileged `notes` user can read it; the server reloads it
# automatically within 12 hours, or immediately on restart.
set -euo pipefail
install -m 640 -o root -g notes "$RENEWED_LINEAGE/fullchain.pem" /etc/notes-server/certs/cert.pem
install -m 640 -o root -g notes "$RENEWED_LINEAGE/privkey.pem"   /etc/notes-server/certs/key.pem
