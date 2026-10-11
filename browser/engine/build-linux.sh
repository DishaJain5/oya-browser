#!/usr/bin/env bash
# Build the patched Linux x64 cloud engine natively on a Linux x64 host (an EC2 Ubuntu 24.04 box),
# from the pins in desktop-source.json. No cross-compiling, so no linux-cross patch is needed.
# Usage: browser/engine/build-linux.sh [work-dir]   (needs ~350 GB free, node, git, curl, python3)
# Output: <work-dir>/oya-native-linux-x64-<version>.tar.gz and its .sha256, ready to pin as
# OYA_NATIVE_LINUX_URL / OYA_NATIVE_LINUX_SHA256 after validating it on Linux.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
root=$(mkdir -p "${1:-$HOME/oya-engine}" && cd "${1:-$HOME/oya-engine}" && pwd)
pin() { node -p "require('$repo/browser/engine/desktop-source.json').$1"; }
version=$(pin engineVersion)
jobs=$(nproc)
export DEPOT_TOOLS_UPDATE=0 PATH="$root/depot_tools:$PATH"
cd "$root"

# Pinned depot_tools.
[ -d depot_tools/.git ] || git init -q depot_tools
git -C depot_tools remote get-url origin >/dev/null 2>&1 ||
  git -C depot_tools remote add origin https://chromium.googlesource.com/chromium/tools/depot_tools.git
git -C depot_tools fetch -q --depth 1 origin "$(pin depotToolsRevision)"
git -C depot_tools checkout -q --detach FETCH_HEAD

# Pinned Electron and the Chromium it selects; gclient's hooks apply Electron's own patches and fetch PGO profiles.
cat > .gclient <<EOF
solutions = [{
  'name': 'src/electron',
  'url': 'https://github.com/electron/electron',
  'deps_file': 'DEPS',
  'managed': False,
  'custom_deps': {'src': 'https://github.com/chromium/chromium.git@$(pin chromiumRevision)'},
  'custom_vars': {'checkout_pgo_profiles': True},
}]
EOF
# Hooks build native test addons, so the toolchain from install-build-deps must exist first.
gclient sync --nohooks --no-history --revision "src/electron@$(pin electronRevision)" -j 16
sudo src/build/install-build-deps.sh --no-prompt --no-arm --no-chromeos-fonts
gclient runhooks

# Oya's overlay, checksum-pinned, applied only to byte-identical baselines.
if [ ! -f overlay/.applied ]; then
  curl --fail --location --proto '=https' --retry 3 -o overlay.tar.gz "$(pin overlayUrl)"
  echo "$(pin overlaySha256)  overlay.tar.gz" | sha256sum --check
  rm -rf overlay && mkdir overlay && tar -xzf overlay.tar.gz -C overlay
  node "$repo/scripts/apply-native-source.mjs" "$root/src" "$root/overlay"
  touch overlay/.applied
fi

# Electron's official release configuration, PGO and ThinLTO included, like the npm Electron binary.
mkdir -p src/out/OyaRelease
cat > src/out/OyaRelease/args.gn <<EOF
import("//electron/build/args/release.gn")
override_electron_version = "$version"
target_cpu = "x64"
symbol_level = 0
generate_about_credits = true
EOF
# The checkout's own gn and ninja: depot_tools' wrappers need a bootstrap DEPOT_TOOLS_UPDATE=0 skips.
# Node's configuration generator calls gn by name, so its directory stays on PATH for ninja.
cd src
export PATH="$PWD/buildtools/linux64:$PATH"
buildtools/linux64/gn gen out/OyaRelease
third_party/ninja/ninja -C out/OyaRelease -j "$jobs" electron:electron_dist_zip

# The archive holds the complete unpacked runtime at its root, as setup-native-engine expects.
cd "$root"
rm -rf distribution && mkdir distribution
unzip -q src/out/OyaRelease/dist.zip -d distribution
archive="oya-native-linux-x64-$version.tar.gz"
tar -czf "$archive" -C distribution .
sha256sum "$archive" | tee "$archive.sha256"
