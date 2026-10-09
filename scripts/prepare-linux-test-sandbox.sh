#!/bin/sh
# CI-only admission for Ubuntu's restricted unprivileged user namespaces.
# Use the distro profile: bwrap can construct its namespaces, while its
# children enter unpriv_bwrap with capabilities denied. Global policy stays on.
set -eu

profile=/usr/share/apparmor/extra-profiles/bwrap-userns-restrict
test -r "$profile"
sudo install -m 0644 "$profile" /etc/apparmor.d/bwrap-userns-restrict
sudo apparmor_parser -r /etc/apparmor.d/bwrap-userns-restrict

# Match the product's isolated namespace/no-network setup before compiling.
# A denied setup is a provisioning failure, never a skipped safety test.
env -i /usr/bin/bwrap --unshare-all --die-with-parent \
  --ro-bind / / --dev /dev --proc /proc --tmpfs /tmp -- /usr/bin/true
