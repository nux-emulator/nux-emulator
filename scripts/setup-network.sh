#!/usr/bin/env bash
#
# Nux Emulator — one-time host network setup for Cuttlefish VM
#
# Run once with sudo to configure networking for the Android VM.
# Detects the host firewall (nftables/iptables/firewalld) and
# configures forwarding + NAT automatically.
#
# Usage: sudo ./scripts/setup-network.sh
#
# This script is idempotent: running it multiple times is safe.
#
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log()  { echo -e "${GREEN}[NUX]${NC} $*"; }
warn() { echo -e "${YELLOW}[NUX]${NC} $*"; }
err()  { echo -e "${RED}[NUX]${NC} $*" >&2; }

if [[ $EUID -ne 0 ]]; then
    err "This script must be run as root: sudo $0"
    exit 1
fi

MAIN_IF=$(ip route | grep default | awk '{print $5}' | head -1)
if [[ -z "$MAIN_IF" ]]; then
    err "Could not detect main network interface"
    exit 1
fi
log "Main network interface: $MAIN_IF"

# ── Enable IP forwarding (persistent) ──
log "Enabling IP forwarding..."
sysctl -qw net.ipv4.ip_forward=1
if ! grep -q "net.ipv4.ip_forward=1" /etc/sysctl.d/99-nux.conf 2>/dev/null; then
    echo "net.ipv4.ip_forward=1" > /etc/sysctl.d/99-nux.conf
    log "  Made persistent via /etc/sysctl.d/99-nux.conf"
fi

# ── Create TAP devices ──
log "Creating TAP devices..."
USER_NAME=${SUDO_USER:-$(whoami)}
for tap in cvd-mtap-01 cvd-etap-01 cvd-wtap-01; do
    if ! ip link show "$tap" &>/dev/null; then
        ip tuntap add dev "$tap" mode tap user "$USER_NAME"
        log "  Created $tap"
    else
        log "  $tap already exists"
    fi
    ip link set "$tap" up
done

# Set gateway IP for OpenWrt WAN
ip addr add 192.168.96.1/24 dev cvd-wtap-01 2>/dev/null || log "  Gateway IP already set"

# ── Detect firewall system ──
FIREWALL="iptables"
if command -v firewall-cmd &>/dev/null && systemctl is-active --quiet firewalld 2>/dev/null; then
    FIREWALL="firewalld"
elif command -v nft &>/dev/null && nft list ruleset 2>/dev/null | grep -q "inet filter"; then
    FIREWALL="nftables"
fi
log "Detected firewall: $FIREWALL"

# ── Configure forwarding + NAT ──
case "$FIREWALL" in
    firewalld)
        log "Configuring firewalld..."
        firewall-cmd --permanent --zone=trusted --add-interface=cvd-mtap-01 2>/dev/null || true
        firewall-cmd --permanent --zone=trusted --add-interface=cvd-etap-01 2>/dev/null || true
        firewall-cmd --permanent --zone=trusted --add-interface=cvd-wtap-01 2>/dev/null || true
        firewall-cmd --permanent --zone=trusted --add-masquerade 2>/dev/null || true
        firewall-cmd --reload
        log "  Added cvd-* interfaces to trusted zone with masquerade"
        ;;

    nftables)
        log "Configuring nftables + iptables..."
        # nftables forwarding
        if ! nft list chain inet filter forward 2>/dev/null | grep -q 'iifname "cvd-\*"'; then
            nft add rule inet filter forward iifname "cvd-*" accept
            nft add rule inet filter forward oifname "cvd-*" ct state established,related accept
            log "  Added nftables forwarding rules"
        else
            log "  nftables forwarding rules already exist"
        fi
        ;;&  # fall through to also add iptables rules

    iptables|nftables)
        log "Configuring iptables..."
        # NAT (idempotent — check before adding)
        for subnet in 192.168.96.0/24 192.168.99.0/24; do
            iptables -t nat -C POSTROUTING -s "$subnet" -o "$MAIN_IF" -j MASQUERADE 2>/dev/null || \
                iptables -t nat -A POSTROUTING -s "$subnet" -o "$MAIN_IF" -j MASQUERADE
        done
        log "  NAT rules configured"

        # FORWARD — INSERT at top (before Docker/other DROP rules)
        iptables -C FORWARD -i cvd-wtap-01 -o "$MAIN_IF" -j ACCEPT 2>/dev/null || \
            iptables -I FORWARD 1 -i cvd-wtap-01 -o "$MAIN_IF" -j ACCEPT
        iptables -C FORWARD -i "$MAIN_IF" -o cvd-wtap-01 -m state --state RELATED,ESTABLISHED -j ACCEPT 2>/dev/null || \
            iptables -I FORWARD 2 -i "$MAIN_IF" -o cvd-wtap-01 -m state --state RELATED,ESTABLISHED -j ACCEPT
        log "  FORWARD rules configured (inserted at top of chain)"
        ;;
esac

echo ""
log "Network setup complete!"
echo ""
log "To verify after starting the emulator:"
log "  adb -s 127.0.0.1:6520 shell ping 8.8.8.8"
echo ""
log "Note: TAP devices and iptables rules are not persistent across reboots."
log "Run this script again after reboot, or add it to a systemd service."
