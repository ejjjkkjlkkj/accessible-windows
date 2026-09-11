#!/usr/bin/env bash
set -euo pipefail

EFI_BINARY="${1:-boot/uefi/target/x86_64-unknown-uefi/release/aw-uefi-boot.efi}"
OUTPUT_IMAGE="${2:-build/accessible-windows-uefi-x86_64.img}"
IMAGE_SIZE_MIB="${IMAGE_SIZE_MIB:-64}"
MOUNT_DIR="$(mktemp -d)"
LOOP_DEVICE=""
MOUNTED=0

cleanup() {
  set +e
  if [ "$MOUNTED" -eq 1 ]; then
    sudo umount "$MOUNT_DIR"
  fi
  if [ -n "$LOOP_DEVICE" ]; then
    sudo losetup -d "$LOOP_DEVICE"
  fi
  rmdir "$MOUNT_DIR" 2>/dev/null || true
}
trap cleanup EXIT

if [ ! -f "$EFI_BINARY" ]; then
  echo "EFI binary not found: $EFI_BINARY" >&2
  exit 1
fi

for tool in sgdisk losetup mkfs.vfat mount; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Required tool not found: $tool" >&2
    exit 1
  fi
done

mkdir -p "$(dirname "$OUTPUT_IMAGE")"
rm -f "$OUTPUT_IMAGE"
truncate -s "${IMAGE_SIZE_MIB}M" "$OUTPUT_IMAGE"

# Create a GPT disk with one EFI System Partition, aligned at 1 MiB.
sgdisk --clear \
  --new=1:2048:0 \
  --typecode=1:EF00 \
  --change-name=1:"Accessible Windows EFI" \
  "$OUTPUT_IMAGE"

LOOP_DEVICE="$(sudo losetup --find --show --partscan "$OUTPUT_IMAGE")"
PARTITION="${LOOP_DEVICE}p1"

# Give the kernel a short window to expose the partition node.
for _ in $(seq 1 50); do
  if [ -b "$PARTITION" ]; then
    break
  fi
  sleep 0.1
done

if [ ! -b "$PARTITION" ]; then
  echo "EFI partition device did not appear: $PARTITION" >&2
  exit 1
fi

sudo mkfs.vfat -F 32 -n AWBOOT "$PARTITION"
sudo mount "$PARTITION" "$MOUNT_DIR"
MOUNTED=1
sudo mkdir -p "$MOUNT_DIR/EFI/BOOT"
sudo cp "$EFI_BINARY" "$MOUNT_DIR/EFI/BOOT/BOOTX64.EFI"
sync
sudo umount "$MOUNT_DIR"
MOUNTED=0
sudo losetup -d "$LOOP_DEVICE"
LOOP_DEVICE=""

sgdisk --verify "$OUTPUT_IMAGE"

echo "AW_DISK_IMAGE_OK path=$OUTPUT_IMAGE size_mib=$IMAGE_SIZE_MIB"
