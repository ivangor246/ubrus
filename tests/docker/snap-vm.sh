#!/bin/bash
# Run the installed strict snap with the guest kernel's snapd, AppArmor and mount namespaces.
set -euo pipefail
image=ubuntu-24.04-minimal-cloudimg-amd64.img
(cd tests/artifacts/vm && grep -F "$image" SHA256SUMS | sha256sum --check --status)
scratch=/tmp/ubrus-guest
mkdir -p "$scratch/assets/tests/session" "$scratch/providers" /tmp/ubrus-seed
cp -r tests/session/{*.py,scenarios,fixtures} "$scratch/assets/tests/session/"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
cp "dist/ubrus_${version}_amd64.snap" "$scratch/assets/ubrus.snap"
# Expose only the three public platform packages that the guest needs.
for name in core24 mesa-2404 gnome-46-2404; do
    provider=$(ls -v /providers/"$name"_*.snap | tail -1)
    cp -u "$provider" "$scratch/providers/$name.snap"
done
[[ -f "$scratch/root.qcow2" ]] || qemu-img create -q -f qcow2 -F qcow2 -b "/src/tests/artifacts/vm/$image" "$scratch/root.qcow2" 8G
printf '%s\n' "instance-id: ubrus-acceptance-$(date +%s)" 'local-hostname: ubrus-acceptance' > /tmp/ubrus-seed/meta-data
cp tests/docker/snap-cloud-init.yaml /tmp/ubrus-seed/user-data
xorrisofs -quiet -output "$scratch/seed.iso" -volid cidata -joliet -rock /tmp/ubrus-seed
set +e
timeout 20m qemu-system-x86_64 -enable-kvm -cpu host -smp 1 -m 2048 -display none -serial stdio -monitor none \
    -no-reboot -drive "file=$scratch/root.qcow2,if=virtio" -drive "file=$scratch/seed.iso,media=cdrom,readonly=on" \
    -nic user,model=virtio-net-pci \
    -virtfs "local,path=$scratch/assets,mount_tag=project,security_model=none,readonly=on" \
    -virtfs "local,path=$scratch/providers,mount_tag=providers,security_model=none,readonly=on" >"$scratch/console" 2>&1
rc=$?
set -e
if [[ $rc != 0 ]] || ! grep -q '^UBRUS_VM_PASS' "$scratch/console"; then
    tr -d '\r' < "$scratch/console" | sed -n '/^UBRUS_TEST_LOG_START/,/^UBRUS_TEST_LOG_END/p' | tail -100
    exit 1
fi
grep -a '^wayland:' "$scratch/console"
grep -q '^UBRUS_VM_PASS' "$scratch/console"
