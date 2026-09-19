#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "usage: $0 <BOOTX64.EFI source> [output.img]" >&2
  exit 2
fi

efi=$1
out=${2:-build/accessible-windows-uefi-screenreader-usb-x86_64.img}

for tool in sgdisk mformat mmd mcopy mdir sha256sum; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "missing required tool: $tool" >&2
    exit 1
  }
done

test -s "$efi" || {
  echo "EFI source missing or empty: $efi" >&2
  exit 1
}

mkdir -p "$(dirname "$out")"
rm -f "$out" "$out.sha256"

# 96 MiB raw disk, GPT, one EFI System Partition starting at 1 MiB.
truncate -s 96M "$out"
sgdisk --zap-all "$out" >/dev/null
sgdisk   --new=1:2048:0   --typecode=1:EF00   --change-name=1:ACCESSIBLE_EFI   "$out" >/dev/null
sgdisk --verify "$out"

first_sector=$(sgdisk --info=1 "$out" | awk -F: '/First sector/ {gsub(/^[[:space:]]+|[[:space:]].*$/, "", $2); print $2}')
if ! [[ "$first_sector" =~ ^[0-9]+$ ]]; then
  echo "unable to resolve EFI partition first sector" >&2
  exit 1
fi
offset=$((first_sector * 512))
image_spec="$out@@$offset"

# FAT32 ESP with the removable-media fallback path required by x64 UEFI.
mformat -i "$image_spec" -F -v ACCESSIBLE ::
mmd -i "$image_spec" ::/EFI
mmd -i "$image_spec" ::/EFI/BOOT
mcopy -i "$image_spec" "$efi" ::/EFI/BOOT/BOOTX64.EFI

cat > /tmp/accessible-windows-usb-readme.txt <<'EOF'
Accessible Windows UEFI screen-reader boot image
Architecture: x86-64
Firmware: UEFI
Boot path: EFI/BOOT/BOOTX64.EFI
Purpose: controlled physical boot validation
Warning: writing this image to a USB device destroys existing data on that device.
EOF
mcopy -i "$image_spec" /tmp/accessible-windows-usb-readme.txt ::/README.TXT
rm -f /tmp/accessible-windows-usb-readme.txt

mdir -i "$image_spec" ::/EFI/BOOT/BOOTX64.EFI
sha256sum "$out" > "$out.sha256"

echo "USB_IMAGE=$out"
echo "USB_IMAGE_SHA256=$(cut -d' ' -f1 "$out.sha256")"
echo "UEFI_SCREENREADER_USB_IMAGE=PASS"
