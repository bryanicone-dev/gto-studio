#!/usr/bin/env bash
# Gera o site estático em dist/ (Linux/macOS e GitHub Actions).
# Requisitos: alvo wasm32-unknown-unknown do Rust e o wasm-bindgen CLI no PATH,
# na mesma versão do crate (veja web/Cargo.lock).
set -euo pipefail
cd "$(dirname "$0")/../.."

cargo build --release --target wasm32-unknown-unknown --manifest-path web/Cargo.toml
rm -rf dist
wasm-bindgen --target web --no-typescript --out-dir dist/pkg web/target/wasm32-unknown-unknown/release/gto_studio_web.wasm
sed "s/const BACKEND = 'server';/const BACKEND = 'wasm';/" static/index.html > dist/index.html
grep -q "const BACKEND = 'wasm';" dist/index.html || { echo "marcador BACKEND não encontrado" >&2; exit 1; }
cp static/worker.js dist/
if [ -f library/index.json ]; then cp -r library dist/library; fi
echo "Pronto: dist/ ($(du -sh dist | cut -f1))"
