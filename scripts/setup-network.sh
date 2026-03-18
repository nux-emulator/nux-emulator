#!/usr/bin/env bash
# setup-network.sh — Create and configure the nux-br0 bridge for TAP networking.
#
# Usage: sudo ./scripts/setup-network.sh
#
# This script is idempotent: running it multiple times is safe.
# It creates a persistent bridge via systemd-networkd that survives reboots.

set -euo pipefail

BRIDGE_NAME="nux-br0"
BRIDGE_SUBNET="192.168.100.0/24"
BRIDGE_IP="192.168.100.1/24"
GUEST_IP="192.168.100.2"
NETWORKD_DIR="/etc/systemd/network"
BRIDGE_NETDEV="$NETWORKD_DIR/50-nux-bridge.netdev"
BRIDGE_NETWORK="$NETWORKD_DIR/50-nux-bridge.network"

# ── Checks ──

if [[ $EUID -ne 0 ]]; then
    echo "error: this script must be run as root (sudo)" >&2
    exit 1
fi

# ── Bridge detection ──

if ip link show "$BRIDGE_NAME" &>/dev/null; then
    echo "bridge '$BRIDGE_NAME' already exists"

    # Verify it's actually a bridge
    if [[ ! -d "/sys/class/net/$BRIDGE_NAME/bridge" ]]; then
        echo "error: '$BRIDGE_NAME' exists but is not a bridge interface" >&2
        exit 1
    fi

    # Verify IP is assigned
    if ip addr show "$BRIDGE_NAME" | grep -q "inet $BRIDGE_IP"; then
        echo "bridge IP $BRIDGE_IP is correctly configured"
    else
        echo "assigning IP $BRIDGE_IP to $BRIDGE_NAME"
        ip addr add "$BRIDGE_IP" dev "$BRIDGE_NAME" 2>/dev/null || true
    fi

    # Ensure it's up
    ip link set "$BRIDGE_NAME" up
else
    echo "creating bridge '$BRIDGE_NAME'"
    ip link add name "$BRIDGE_NAME" type bridge
    ip addr add "$BRIDGE_IP" dev "$BRIDGE_NAME"
    ip link set "$BRIDGE_NAME" up
    echo "bridge '$BRIDGE_NAME' created with IP $BRIDGE_IP"
fi

# ── NAT / IP forwarding ──

echo "enabling IP forwarding"
sysctl -q -w net.ipv4.ip_forward=1

# Persist IP forwarding
if ! grep -q "^net.ipv4.ip_forward=1" /etc/sysctl.d/99-nux-forward.conf 2>/dev/null; then
    echo "net.ipv4.ip_forward=1" > /etc/sysctl.d/99-nux-forward.conf
fi

# Add iptables MASQUERADE rule (idempotent)
if ! iptables -t nat -C POSTROUTING -s "$BRIDGE_SUBNET" ! -o "$BRIDGE_NAME" -j MASQUERADE 2>/dev/null; then
    echo "adding NAT masquerade rule for $BRIDGE_SUBNET"
    iptables -t nat -A POSTROUTING -s "$BRIDGE_SUBNET" ! -o "$BRIDGE_NAME" -j MASQUERADE
fi

# Allow forwarding to/from the bridge
if ! iptables -C FORWARD -i "$BRIDGE_NAME" -j ACCEPT 2>/dev/null; then
    iptables -A FORWARD -i "$BRIDGE_NAME" -j ACCEPT
fi
if ! iptables -C FORWARD -o "$BRIDGE_NAME" -m state --state RELATED,ESTABLISHED -j ACCEPT 2>/dev/null; then
    iptables -A FORWARD -o "$BRIDGE_NAME" -m state --state RELATED,ESTABLISHED -j ACCEPT
fi

# ── systemd-networkd persistence ──

echo "writing systemd-networkd drop-in files"
mkdir -p "$NETWORKD_DIR"

cat > "$BRIDGE_NETDEV" <<EOF
[NetDev]
Name=$BRIDGE_NAME
Kind=bridge
EOF

cat > "$BRIDGE_NETWORK" <<EOF
[Match]
Name=$BRIDGE_NAME

[Network]
Address=$BRIDGE_IP
DHCPServer=yes
IPMasquerade=ipv4

[DHCPServer]
PoolOffset=2
PoolSize=100
DNS=8.8.8.8
DNS=8.8.4.4
EmitDNS=yes
EOF

# Reload networkd if it's running
if systemctl is-active --quiet systemd-networkd; then
    systemctl reload systemd-networkd || systemctl restart systemd-networkd
    echo "systemd-networkd reloaded"
fi

echo ""
echo "setup complete:"
echo "  bridge:    $BRIDGE_NAME ($BRIDGE_IP)"
echo "  subnet:    $BRIDGE_SUBNET"
echo "  guest IP:  $GUEST_IP (via DHCP)"
echo "  DNS:       8.8.8.8, 8.8.4.4 (forwarded to guest)"
echo "  NAT:       enabled"
echo "  persist:   systemd-networkd ($BRIDGE_NETDEV, $BRIDGE_NETWORK)"
