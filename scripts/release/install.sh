#!/usr/bin/env bash
set -euo pipefail
# CodeWhale Unix installer
# Copies codewhale and codew to ~/.local/bin (or $PREFIX/bin)

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="${PREFIX}/bin"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

version_code() {
    local version="$1"
    local major minor patch
    IFS=. read -r major minor patch <<< "$version"
    printf '%d%03d%03d\n' "${major:-0}" "${minor:-0}" "${patch:-0}"
}

detect_host_glibc() {
    local out
    if out="$(getconf GNU_LIBC_VERSION 2>/dev/null)"; then
        printf '%s\n' "$out" | awk '{print $NF; exit}'
        return 0
    fi
    if out="$(ldd --version 2>&1 | head -n 1)"; then
        printf '%s\n' "$out" | grep -Eo '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -n 1
        return 0
    fi
    return 1
}

required_glibc_for_binary() {
    local bin="$1"
    local versions
    versions="$(grep -aoE 'GLIBC_[0-9]+\.[0-9]+(\.[0-9]+)?' "$bin" 2>/dev/null | sed 's/^GLIBC_//' || true)"
    if [[ -z "$versions" ]]; then
        return 1
    fi
    printf '%s\n' "$versions" | awk -F. '
        {
            patch = ($3 == "" ? 0 : $3)
            code = ($1 * 1000000) + ($2 * 1000) + patch
            if (code > best) {
                best = code
                value = $0
            }
        }
        END {
            if (value != "") print value
        }
    '
}

preflight_glibc() {
    local bin="$1"
    if [[ "$(uname -s)" != "Linux" ]]; then
        return 0
    fi
    if [[ "${CODEWHALE_SKIP_GLIBC_CHECK:-}" == "1" || "${DEEPSEEK_TUI_SKIP_GLIBC_CHECK:-}" == "1" || "${DEEPSEEK_SKIP_GLIBC_CHECK:-}" == "1" ]]; then
        return 0
    fi

    local required
    if ! required="$(required_glibc_for_binary "$bin")" || [[ -z "$required" ]]; then
        return 0
    fi

    local host
    if ! host="$(detect_host_glibc)" || [[ -z "$host" ]]; then
        echo "ERROR: $(basename "$bin") requires GLIBC_$required, but no GNU libc was detected." >&2
        echo "Official Codewhale Linux release assets (x64 and arm64) are static musl builds" >&2
        echo "with no glibc dependency, so this binary is not an official release asset." >&2
        echo "Check where it came from, or build from source on this host." >&2
        echo "Build from source instead: cargo install codewhale-cli --locked" >&2
        echo "Set CODEWHALE_SKIP_GLIBC_CHECK=1 to bypass this check at your own risk." >&2
        return 1
    fi

    if [[ "$(version_code "$host")" -lt "$(version_code "$required")" ]]; then
        echo "ERROR: $(basename "$bin") requires GLIBC_$required, but this system has glibc $host." >&2
        echo "Official Codewhale Linux release assets (x64 and arm64) are static musl builds" >&2
        echo "with no glibc dependency, so this binary is not an official release asset." >&2
        echo "Check where it came from, or build from source on this host." >&2
        echo "Build from source instead: cargo install codewhale-cli --locked" >&2
        echo "Set CODEWHALE_SKIP_GLIBC_CHECK=1 to bypass this check at your own risk." >&2
        return 1
    fi
}

# This script installs an already checksum-verified archive. Existing different
# binaries go through `codewhale update`, the sole version-aware updater.
case "$BIN_DIR" in
    /*) ;;
    *) echo "ERROR: PREFIX must be an absolute user path" >&2; exit 1 ;;
esac
[[ ! -L "$BIN_DIR" ]] || { echo "ERROR: $BIN_DIR is a symlink; choose a fresh PREFIX" >&2; exit 1; }
mkdir -p "$BIN_DIR"
BIN_DIR="$(cd -P "$BIN_DIR" && pwd)"
case "$BIN_DIR/" in
    /bin/*|/sbin/*|/usr/bin/*|/usr/sbin/*|/nix/store/*|/gnu/store/*|*/node_modules/*|*/Cellar/*|*/.linuxbrew/*|*/linuxbrew/*|*/.cargo/bin/*)
        echo "ERROR: refusing managed/system directory $BIN_DIR; choose a fresh user PREFIX" >&2
        exit 1
        ;;
esac
[[ -w "$BIN_DIR" ]] || { echo "ERROR: $BIN_DIR is not writable; choose a user PREFIX (no sudo)" >&2; exit 1; }

check_destination() {
    local src="$1" dst="$2"
    destination_exists=0
    if [[ -e "$dst" || -L "$dst" ]]; then
        if [[ ! -L "$dst" && -f "$dst" && -x "$dst" ]] && cmp -s "$src" "$dst"; then
            destination_exists=1
            return 0
        fi
        echo "ERROR: refusing to replace existing $dst; no existing file was changed." >&2
        echo "For an existing direct Codewhale install, run its full path with 'update'." >&2
        echo "To migrate, install this verified archive into a fresh user prefix:" >&2
        echo '  mkdir -p "$HOME/.local"' >&2
        echo '  codewhale_prefix="$(mktemp -d "$HOME/.local/codewhale-release.XXXXXX")"' >&2
        echo '  PREFIX="$codewhale_prefix" ./install.sh' >&2
        echo '  "$codewhale_prefix/bin/codewhale" --version' >&2
        echo "Put the selected prefix/bin first on PATH after verifying it; see docs/INSTALL.md." >&2
        return 1
    fi
}

# Validate both sources and every destination before the first write.
for bin in codewhale codew; do
    src="$SCRIPT_DIR/$bin"
    [[ -f "$src" ]] || { echo "ERROR: $src not found in archive" >&2; exit 1; }
    preflight_glibc "$src"
    check_destination "$src" "$BIN_DIR/$bin"
done
legacy_tui="$BIN_DIR/codewhale-tui"
if [[ -e "$legacy_tui" || -L "$legacy_tui" ]]; then
    check_destination "$SCRIPT_DIR/codewhale" "$legacy_tui"
fi

stage=""
stage_dir=""
trap 'if [[ -n "$stage" ]]; then rm -f "$stage"; fi; if [[ -n "$stage_dir" ]]; then rmdir "$stage_dir"; fi' EXIT
install_binary() {
    local src="$1" dst="$2"
    check_destination "$src" "$dst"
    if [[ "$destination_exists" == 1 ]]; then
        echo "  $dst (already installed)"
        return
    fi
    stage_dir="$(mktemp -d "$BIN_DIR/.codewhale-install.XXXXXX")"
    stage="$stage_dir/$(basename "$dst")"
    cp "$src" "$stage"
    chmod 0755 "$stage"
    # Same-directory hard-link publication is atomic and never overwrites a
    # destination created between preflight and this operation.
    # The explicit parent operand avoids treating a raced-in destination
    # directory (or directory symlink) as an alternate publication location.
    ln "$stage" "$BIN_DIR/"
    [[ ! -L "$dst" && -f "$dst" ]] && cmp -s "$stage" "$dst" || {
        echo "ERROR: installed path changed during publication: $dst" >&2
        return 1
    }
    rm -f "$stage"
    rmdir "$stage_dir"
    stage=""
    stage_dir=""
    echo "  $dst"
}

echo "Installing codewhale to $BIN_DIR ..."
for bin in codewhale codew; do
    install_binary "$SCRIPT_DIR/$bin" "$BIN_DIR/$bin"
done

echo ""
echo "Done. Commands installed to $BIN_DIR."
echo "Future updates: \"$BIN_DIR/codewhale\" update"
for bin in codewhale codew; do
    resolved="$(command -v "$bin" || true)"
    if [[ "$resolved" != "$BIN_DIR/$bin" ]]; then
        echo "PATH selects ${resolved:-no $bin command}; this install is $BIN_DIR/$bin"
    fi
done
echo "To select this installation in the current shell:"
echo "  export PATH=\"$BIN_DIR:\$PATH\""
echo "  hash -r"
echo "  command -v codewhale codew"
echo "Keep the directory first in your shell profile after verifying it."
if ! command -v node >/dev/null 2>&1; then
    echo "Computer Use is included and needs Node.js 20 or newer on PATH."
    echo "Install Node.js from https://nodejs.org/, then restart Codewhale to enable Computer Use."
fi
