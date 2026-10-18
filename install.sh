#!/bin/sh
set -e
BIN_DIR="${HOME}/.local/bin"
mkdir -p "$BIN_DIR"
echo "Downloading OrquestraAI..."
curl -fsSL https://github.com/internetdozero/OrquestraAI/releases/latest/download/orquestra-ai -o "${BIN_DIR}/orquestra"
chmod +x "${BIN_DIR}/orquestra"
echo "✓ OrquestraAI installed successfully to ${BIN_DIR}/orquestra"
