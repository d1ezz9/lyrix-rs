#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${HOME}/.local"
ASSUME_YES=0
SKIP_DEPS=0
if [[ -r /etc/os-release ]]; then . /etc/os-release; fi

usage() {
    cat <<'EOF'
Usage: ./install.sh [options]

Build and install lyrix. The default destination is ~/.local/bin/lyrix;
no root access is needed unless playerctl must be installed system-wide.

Options:
  --prefix DIR  Installation prefix (binary goes to DIR/bin/lyrix)
  --system      Install to /usr/local (may require sudo)
  --yes, -y     Install missing playerctl automatically using the detected package manager
  --skip-deps   Do not install/check playerctl; useful for custom environments
  --help, -h    Show this help
EOF
}

while (($#)); do
    case "$1" in
        --prefix)
            [[ $# -ge 2 ]] || { echo "--prefix requires a directory" >&2; exit 2; }
            PREFIX="$2"
            shift 2
            ;;
        --system) PREFIX="/usr/local"; shift ;;
        --yes|-y) ASSUME_YES=1; shift ;;
        --skip-deps) SKIP_DEPS=1; shift ;;
        --help|-h) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

if [[ ! -f "$SCRIPT_DIR/Cargo.toml" ]]; then
    echo "Run this installer from a lyrix source checkout." >&2
    exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
    cat >&2 <<'EOF'
Rust/Cargo was not found. Install the Rust toolchain first (for example, from
https://rustup.rs), then rerun this installer.
EOF
    exit 1
fi

run_as_root() {
    if [[ ${EUID:-$(id -u)} -eq 0 ]]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo "$@"
    else
        echo "This step needs root privileges; install sudo or run as root." >&2
        return 1
    fi
}

package_manager() {
    local ids="${ID:-} ${ID_LIKE:-}"
    if [[ "$ids" =~ (debian|ubuntu|linuxmint|pop) ]] && command -v apt-get >/dev/null 2>&1; then echo apt; return; fi
    if [[ "$ids" =~ (fedora|rhel|centos|rocky|alma) ]] && command -v dnf >/dev/null 2>&1; then echo dnf; return; fi
    if [[ "$ids" =~ (arch|manjaro|endeavouros) ]] && command -v pacman >/dev/null 2>&1; then echo pacman; return; fi
    if [[ "$ids" =~ (opensuse|suse) ]] && command -v zypper >/dev/null 2>&1; then echo zypper; return; fi
    if [[ "$ids" =~ (void) ]] && command -v xbps-install >/dev/null 2>&1; then echo xbps; return; fi
    if [[ "$ids" =~ (alpine) ]] && command -v apk >/dev/null 2>&1; then echo apk; return; fi
    for candidate in apt dnf pacman zypper xbps apk; do
        case "$candidate" in
            apt) command -v apt-get >/dev/null 2>&1 && { echo apt; return; } ;;
            dnf|pacman|zypper|apk) command -v "$candidate" >/dev/null 2>&1 && { echo "$candidate"; return; } ;;
            xbps) command -v xbps-install >/dev/null 2>&1 && { echo xbps; return; } ;;
        esac
    done
    return 1
}

install_playerctl() {
    local manager
    manager="$(package_manager)" || {
        echo "No supported package manager found. Install playerctl manually, then rerun ./install.sh." >&2
        return 1
    }
    echo "Installing missing dependency: playerctl (package manager: $manager)"
    case "$manager" in
        apt) run_as_root apt-get install -y playerctl ;;
        dnf) run_as_root dnf install -y playerctl ;;
        pacman) run_as_root pacman -S --needed --noconfirm playerctl ;;
        zypper) run_as_root zypper --non-interactive install playerctl ;;
        xbps) run_as_root xbps-install -y playerctl ;;
        apk) run_as_root apk add playerctl ;;
    esac
}

if (( ! SKIP_DEPS )) && ! command -v playerctl >/dev/null 2>&1; then
    if (( ! ASSUME_YES )); then
        if [[ ! -t 0 ]]; then
            echo "playerctl is missing and stdin is not interactive. Rerun with --yes to install it, or --skip-deps to skip." >&2
            exit 1
        fi
        read -r -p "playerctl is required but missing. Install it using the system package manager? [y/N] " answer
        case "$answer" in y|Y|yes|YES) ;; *) echo "Cancelled. Install playerctl manually or rerun with --skip-deps."; exit 1 ;; esac
    fi
    install_playerctl
fi

if ! command -v playerctl >/dev/null 2>&1 && (( ! SKIP_DEPS )); then
    echo "playerctl is still unavailable after installation." >&2
    exit 1
fi

if ! mkdir -p "$PREFIX/bin" 2>/dev/null; then
    run_as_root mkdir -p "$PREFIX/bin"
fi
echo "Building optimized Rust binary..."
(cd "$SCRIPT_DIR" && cargo build --release)

DEST="$PREFIX/bin/lyrix"
if [[ -w "$PREFIX/bin" ]]; then
    install -m 755 "$SCRIPT_DIR/target/release/lyrix" "$DEST"
else
    run_as_root install -m 755 "$SCRIPT_DIR/target/release/lyrix" "$DEST"
fi

echo "Installed: $DEST"
case ":${PATH}:" in
    *":$PREFIX/bin:"*) ;;
    *) echo "Note: $PREFIX/bin is not currently in PATH. Add it to your shell PATH to run 'lyrix'." ;;
esac
if [[ -x /usr/bin/lyrix && "$DEST" != /usr/bin/lyrix ]]; then
    echo "Note: legacy /usr/bin/lyrix exists and may take precedence over this install, depending on PATH. Remove it manually if it is the old Python version."
fi
echo "Run 'lyrix' to start; press q to quit."
