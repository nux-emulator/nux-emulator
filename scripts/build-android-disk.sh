#!/bin/bash
# Build a GPT disk image with all partitions Android needs.
# This replaces Cuttlefish's composite disk builder.
# Run once during bootstrap — the resulting image is used directly by QEMU.

set -e

DATA_DIR="${HOME}/.local/share/nux-emulator"
PRODUCT_OUT="/build2/nux-emulator/nux-android-image/aosp/out/target/product/vsoc_x86_64"
DISK="${DATA_DIR}/android_disk.img"

if [ -f "$DISK" ]; then
    echo "Disk already exists: $DISK"
    exit 0
fi

echo "=== Building Android GPT disk image ==="
mkdir -p "$DATA_DIR"

# Partition sizes in MB
MISC_MB=1
BOOT_MB=64
INIT_BOOT_MB=8
VENDOR_BOOT_MB=64
VBMETA_MB=1  # Round up from 64KB for alignment
SUPER_MB=7168
USERDATA_MB=16384  # 64GB
METADATA_MB=64

# Calculate total size (partitions + GPT overhead)
# GPT header: 1MB at start, 1MB at end
TOTAL_MB=$((1 + MISC_MB + BOOT_MB*2 + INIT_BOOT_MB*2 + VENDOR_BOOT_MB*2 + VBMETA_MB*8 + SUPER_MB + USERDATA_MB + METADATA_MB + 1))
echo "Total disk size: ${TOTAL_MB} MB (sparse)"

# Create sparse disk image
truncate -s "${TOTAL_MB}M" "$DISK"

# Create GPT partition table with sgdisk
# Each partition gets a name that Android's init uses via /dev/block/by-name/
echo "Creating GPT partition table..."

SECTOR=512
START=2048  # Start at 1MB (sector 2048)

add_partition() {
    local num=$1
    local name=$2
    local size_mb=$3
    local end=$((START + size_mb * 1024 * 1024 / SECTOR - 1))
    sgdisk -n "${num}:${START}:${end}" -c "${num}:${name}" "$DISK" > /dev/null
    echo "  Partition ${num}: ${name} (${size_mb}MB) sectors ${START}-${end}"
    START=$((end + 1))
}

add_partition 1 "misc" $MISC_MB
add_partition 2 "boot_a" $BOOT_MB
add_partition 3 "boot_b" $BOOT_MB
add_partition 4 "init_boot_a" $INIT_BOOT_MB
add_partition 5 "init_boot_b" $INIT_BOOT_MB
add_partition 6 "vendor_boot_a" $VENDOR_BOOT_MB
add_partition 7 "vendor_boot_b" $VENDOR_BOOT_MB
add_partition 8 "vbmeta_a" $VBMETA_MB
add_partition 9 "vbmeta_b" $VBMETA_MB
add_partition 10 "vbmeta_system_a" $VBMETA_MB
add_partition 11 "vbmeta_system_b" $VBMETA_MB
add_partition 12 "vbmeta_vendor_dlkm_a" $VBMETA_MB
add_partition 13 "vbmeta_vendor_dlkm_b" $VBMETA_MB
add_partition 14 "vbmeta_system_dlkm_a" $VBMETA_MB
add_partition 15 "vbmeta_system_dlkm_b" $VBMETA_MB
add_partition 16 "super" $SUPER_MB
add_partition 17 "userdata" $USERDATA_MB
add_partition 18 "metadata" $METADATA_MB

echo "Writing partition data..."

# Helper: write image to partition offset
write_partition() {
    local part_num=$1
    local src=$2
    # Get partition start sector from sgdisk
    local start_sector=$(sgdisk -i "$part_num" "$DISK" 2>/dev/null | grep "First sector" | awk '{print $3}')
    local offset=$((start_sector * SECTOR))
    if [ -f "$src" ]; then
        echo "  Writing $src -> partition $part_num (offset $offset)"
        dd if="$src" of="$DISK" bs=1M seek=$((offset / 1048576)) conv=notrunc status=none 2>/dev/null || \
        dd if="$src" of="$DISK" bs=$SECTOR seek=$start_sector conv=notrunc status=none
    else
        echo "  SKIP: $src not found"
    fi
}

# Write AOSP images to their partitions (A slot)
write_partition 2 "${PRODUCT_OUT}/boot.img"
write_partition 3 "${PRODUCT_OUT}/boot.img"
write_partition 4 "${PRODUCT_OUT}/init_boot.img"
write_partition 5 "${PRODUCT_OUT}/init_boot.img"
write_partition 6 "${PRODUCT_OUT}/vendor_boot.img"
write_partition 7 "${PRODUCT_OUT}/vendor_boot.img"
write_partition 8 "${PRODUCT_OUT}/vbmeta.img"
write_partition 9 "${PRODUCT_OUT}/vbmeta.img"
write_partition 10 "${PRODUCT_OUT}/vbmeta_system.img"
write_partition 11 "${PRODUCT_OUT}/vbmeta_system.img"
write_partition 12 "${PRODUCT_OUT}/vbmeta_vendor_dlkm.img"
write_partition 13 "${PRODUCT_OUT}/vbmeta_vendor_dlkm.img"
write_partition 14 "${PRODUCT_OUT}/vbmeta_system_dlkm.img"
write_partition 15 "${PRODUCT_OUT}/vbmeta_system_dlkm.img"
write_partition 16 "${PRODUCT_OUT}/super.img"

# Format userdata as f2fs
echo "Formatting userdata partition as f2fs..."
USERDATA_START=$(sgdisk -i 17 "$DISK" 2>/dev/null | grep "First sector" | awk '{print $3}')
USERDATA_SIZE=$(sgdisk -i 17 "$DISK" 2>/dev/null | grep "Partition size" | awk '{print $3}')
# Create a separate userdata.img and write it
USERDATA_IMG="${DATA_DIR}/userdata_tmp.img"
truncate -s "${USERDATA_MB}M" "$USERDATA_IMG"
mkfs.f2fs -f -l data "$USERDATA_IMG"
write_partition 17 "$USERDATA_IMG"
rm -f "$USERDATA_IMG"

# Format metadata as ext4
echo "Formatting metadata partition as ext4..."
METADATA_IMG="${DATA_DIR}/metadata_tmp.img"
truncate -s "${METADATA_MB}M" "$METADATA_IMG"
mkfs.ext4 -F "$METADATA_IMG"
write_partition 18 "$METADATA_IMG"
rm -f "$METADATA_IMG"

echo "=== Done: $DISK ==="
ls -lh "$DISK"
echo "Actual disk usage:"
du -h "$DISK"
