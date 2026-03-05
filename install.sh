#!/usr/bin/env bash
set -euo pipefail

BIN_DIR="${HOME}/.local/bin"
BIN_NAME="claude-continue"
ALIAS_NAME="cc"

echo "Building claude-continue (release)..."
cargo build --release

mkdir -p "$BIN_DIR"
cp "target/release/${BIN_NAME}" "${BIN_DIR}/${BIN_NAME}"
chmod +x "${BIN_DIR}/${BIN_NAME}"
echo "Installed ${BIN_DIR}/${BIN_NAME}"

# Add alias to shell profile if not already present
add_alias() {
    local file="$1"
    if [[ -f "$file" ]] && grep -q "alias ${ALIAS_NAME}=" "$file" 2>/dev/null; then
        echo "Alias '${ALIAS_NAME}' already exists in ${file}"
        return
    fi
    # Pick the first existing profile file
    if [[ -f "$file" ]]; then
        echo "" >> "$file"
        echo "# claude-continue" >> "$file"
        echo "alias ${ALIAS_NAME}=${BIN_NAME}" >> "$file"
        echo "Added alias '${ALIAS_NAME}' to ${file}"
        return 0
    fi
    return 1
}

# Try common shell profiles
if [[ -n "${ZSH_VERSION:-}" ]] || [[ "$SHELL" == */zsh ]]; then
    add_alias "${HOME}/.zshrc"
elif [[ -n "${BASH_VERSION:-}" ]] || [[ "$SHELL" == */bash ]]; then
    add_alias "${HOME}/.bashrc" || add_alias "${HOME}/.bash_profile"
else
    add_alias "${HOME}/.bashrc" || add_alias "${HOME}/.zshrc" || add_alias "${HOME}/.profile"
fi

echo ""
echo "Done! Run 'source ~/.bashrc' (or ~/.zshrc) then use '${ALIAS_NAME}' or '${BIN_NAME}'."
