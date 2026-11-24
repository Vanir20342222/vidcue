#!/bin/sh
# Installs or upgrades vidcue-server on a Debian 13 host with local PostgreSQL.
# Usage (as root, from a directory containing the binary and this deploy folder):
#   PUBLIC_URL=https://vidcue.example.org PROXY_IP=192.168.1.5 sh install.sh ./vidcue-server
set -eu
bin=${1:?path to vidcue-server binary}
here=$(dirname "$0")
: "${PUBLIC_URL:?set PUBLIC_URL}"
: "${PROXY_IP:?set PROXY_IP (the reverse proxy that terminates TLS)}"
BIND=${BIND:-0.0.0.0:8080}
PORT=${BIND##*:}

id vidcue >/dev/null 2>&1 || useradd --system --home-dir /nonexistent --shell /usr/sbin/nologin vidcue
# Peer authentication over the local socket: no database password exists.
su postgres -c "psql -tAc \"SELECT 1 FROM pg_roles WHERE rolname='vidcue'\"" | grep -q 1 ||
  su postgres -c "createuser vidcue"
su postgres -c "psql -tAc \"SELECT 1 FROM pg_database WHERE datname='vidcue'\"" | grep -q 1 ||
  su postgres -c "createdb --owner=vidcue vidcue"

install -d -m 750 -o root -g vidcue /etc/vidcue
env=/etc/vidcue/server.env
[ -f "$env" ] || install -m 640 -o root -g vidcue /dev/null "$env"
set_env() {
  if grep -q "^$1=" "$env"; then
    sed -i "s|^$1=.*|$1=$2|" "$env"
  else
    echo "$1=$2" >> "$env"
  fi
}
grep -q '^DATABASE_URL=' "$env" || set_env DATABASE_URL "host=/run/postgresql user=vidcue dbname=vidcue"
set_env VIDCUE_PUBLIC_URL "$PUBLIC_URL"
set_env VIDCUE_BIND "$BIND"
set_env VIDCUE_TRUSTED_PROXIES "$PROXY_IP"
set_env VIDCUE_PROXIES_ONLY true
grep -q '^RUST_LOG=' "$env" || set_env RUST_LOG info

install -m 755 "$bin" /usr/local/bin/vidcue-server.new
mv /usr/local/bin/vidcue-server.new /usr/local/bin/vidcue-server

# Server key: generated once, root-only, passed in as a systemd credential. Losing it
# makes stored authenticator secrets unreadable, so back it up separately from the database.
if [ ! -s /etc/vidcue/secret.key ]; then
  (umask 077; /usr/local/bin/vidcue-server gen-key > /etc/vidcue/secret.key)
fi
chown root:root /etc/vidcue/secret.key
chmod 400 /etc/vidcue/secret.key

# Only the reverse proxy may reach the service port.
if command -v nft >/dev/null 2>&1; then
  cat > /etc/nftables.d-vidcue.nft <<EOF
table inet vidcue {
  chain input {
    type filter hook input priority 0; policy accept;
    tcp dport $PORT ip saddr != { 127.0.0.1, $PROXY_IP } drop
    tcp dport $PORT ip6 saddr != ::1 drop
  }
}
EOF
  nft delete table inet vidcue 2>/dev/null || true
  nft -f /etc/nftables.d-vidcue.nft
  grep -q nftables.d-vidcue /etc/nftables.conf 2>/dev/null ||
    echo 'include "/etc/nftables.d-vidcue.nft"' >> /etc/nftables.conf
  systemctl enable nftables >/dev/null 2>&1 || true
fi

install -m 644 "$here/vidcue-server.service" /etc/systemd/system/vidcue-server.service
install -m 755 "$here/vidcue-backup.sh" /usr/local/sbin/vidcue-backup
cat > /etc/systemd/system/vidcue-backup.service <<'EOF'
[Unit]
Description=Back up the Vidcue database
[Service]
Type=oneshot
UMask=0077
ExecStart=/usr/local/sbin/vidcue-backup
EOF
cat > /etc/systemd/system/vidcue-backup.timer <<'EOF'
[Unit]
Description=Nightly Vidcue database backup
[Timer]
OnCalendar=*-*-* 03:30:00
RandomizedDelaySec=15m
Persistent=true
[Install]
WantedBy=timers.target
EOF
systemctl daemon-reload
systemctl enable --now vidcue-backup.timer
systemctl enable vidcue-server
systemctl restart vidcue-server
systemctl --no-pager --lines=5 status vidcue-server
