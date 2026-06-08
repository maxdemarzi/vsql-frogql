#!/usr/bin/env bash
# Build the Rust VillageSQL extension and package it as a .veb archive.

set -euo pipefail

: "${VillageSQL_BUILD_DIR:?VillageSQL_BUILD_DIR must be set}"

echo "Building vsql_frogql extension with cargo..."
cargo build --release

# Locate compiled library
LIB_PATH="target/release/libvsql_frogql.so"
if [[ ! -f "$LIB_PATH" ]]; then
    echo "Error: compiled library not found at $LIB_PATH" >&2
    exit 1
fi

# Stage VEB contents in a temp directory
STAGING=$(mktemp -d)
trap 'rm -rf "$STAGING"' EXIT

mkdir -p "$STAGING/lib"
cp "$LIB_PATH" "$STAGING/lib/vsql_frogql.so"
cp manifest.json "$STAGING/manifest.json"

# Create the .veb archive
VEB_DIR="build"
mkdir -p "$VEB_DIR"
VEB="$VEB_DIR/vsql_frogql.veb"
tar -C "$STAGING" -cf "$VEB" manifest.json lib/
echo "Created: $VEB"

# Install into the VillageSQL build tree
INSTALL_DIR="$VillageSQL_BUILD_DIR/veb_output_directory"
mkdir -p "$INSTALL_DIR"
cp "$VEB" "$INSTALL_DIR/"
echo "Installed to: $INSTALL_DIR/vsql_frogql.veb"
