#!/usr/bin/env bash
set -euo pipefail

BINARY="nymphalis"
INSTALL_DIR="${HOME}/.local/bin"

echo "==> Building ${BINARY} (release)..."
cargo build --release

mkdir -p "${INSTALL_DIR}"
cp "target/release/${BINARY}" "${INSTALL_DIR}/${BINARY}"
chmod +x "${INSTALL_DIR}/${BINARY}"
echo "==> Installed: ${INSTALL_DIR}/${BINARY}"

# Source the appropriate shell config so the new binary is on PATH immediately.
# Note: sourcing only takes effect in the calling shell when this script is
# itself sourced (`. ./install.sh`). For fish — which cannot source bash
# scripts — the command to run is printed instead.
_SHELL="$(basename "${SHELL:-bash}")"
case "${_SHELL}" in
    bash)
        _CFG="${HOME}/.bashrc"
        if [ -f "${_CFG}" ]; then
            # shellcheck disable=SC1090
            . "${_CFG}"
            echo "==> Sourced ${_CFG}"
        fi
        ;;
    zsh)
        _CFG="${HOME}/.zshrc"
        if [ -f "${_CFG}" ]; then
            # shellcheck disable=SC1090
            . "${_CFG}"
            echo "==> Sourced ${_CFG}"
        fi
        ;;
    fish)
        # fish_add_path writes to universal variables — persists across sessions
        # without needing to edit config.fish. Takes effect in the current session
        # after sourcing config (or opening a new tab).
        fish -c "fish_add_path '${INSTALL_DIR}'"
        echo "==> Added ${INSTALL_DIR} to fish_user_paths (universal)"
        echo "==> Refresh current session:"
        echo "    source ~/.config/fish/config.fish"
        ;;
    *)
        echo "==> Unknown shell '${_SHELL}'. Ensure ${INSTALL_DIR} is in your PATH."
        ;;
esac

echo "==> Done. Try: ${BINARY} --help"
