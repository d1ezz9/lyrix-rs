#!/usr/bin/env bash
set -Eeuo pipefail

PREFIX="${HOME}/.local"
LEGACY=0

usage() {
    cat <<'EOF'
Usage: ./uninstall.sh [options]

Options:
  --prefix DIR  Remove DIR/bin/lyrix (default: ~/.local/bin/lyrix)
  --system      Remove /usr/local/bin/lyrix
  --legacy      Also ask before removing the old /usr/bin/lyrix installation
  --help, -h    Show this help
EOF
}

while (($#)); do
    case "$1" in
        --prefix) [[ $# -ge 2 ]] || { echo "--prefix requires a directory" >&2; exit 2; }; PREFIX="$2"; shift 2 ;;
        --system) PREFIX="/usr/local"; shift ;;
        --legacy) LEGACY=1; shift ;;
        --help|-h) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

remove_file() {
    local target="$1"
    [[ -e "$target" || -L "$target" ]] || { echo "Not installed: $target"; return 0; }
    if [[ -w "$(dirname -- "$target")" ]]; then
        rm -f -- "$target"
    elif command -v sudo >/dev/null 2>&1; then
        sudo rm -f -- "$target"
    else
        echo "Cannot remove $target without root privileges (sudo not found)." >&2
        return 1
    fi
    echo "Removed: $target"
}

remove_file "$PREFIX/bin/lyrix"
if (( LEGACY )) && [[ -e /usr/bin/lyrix || -L /usr/bin/lyrix ]]; then
    if [[ ! -t 0 ]]; then
        echo "Legacy /usr/bin/lyrix found. Run interactively to confirm its removal." >&2
        exit 1
    fi
    read -r -p "Remove legacy /usr/bin/lyrix too? [y/N] " answer
    case "$answer" in y|Y|yes|YES) remove_file /usr/bin/lyrix ;; *) echo "Kept /usr/bin/lyrix" ;; esac
fi
