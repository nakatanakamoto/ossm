#!/usr/bin/env bash
set -euo pipefail

crate="$1"

if [ ! -f "firmware/${crate}/Cargo.toml" ]; then
    echo "Error: firmware/${crate}/Cargo.toml does not exist"
    exit 1
fi

jq ". + {\"rust-analyzer.linkedProjects\": [\"firmware/${crate}/Cargo.toml\"]}" \
    .vscode/settings.template.json > .vscode/settings.json

jq ".lsp[\"rust-analyzer\"].initialization_options.linkedProjects = [\"firmware/${crate}/Cargo.toml\"]" \
    .zed/settings.template.json > .zed/settings.json

echo "rust-analyzer focused on ${crate}"
