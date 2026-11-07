#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

version="$(sed -n 's/^version = "\(.*\)"/\1/p' plugin.toml | head -n 1)"
name="laravel-openidea-${version}"
out="build/${name}"

cargo build --release --target wasm32-wasip2
mkdir -p dist
cp target/wasm32-wasip2/release/openidea_plugin_laravel_openidea.wasm dist/plugin.wasm

rm -rf build
mkdir -p "${out}/dist"
cp plugin.toml settings.schema.json LICENSE README.md "${out}/"
cp dist/plugin.wasm "${out}/dist/"
cp -r icons languages templates "${out}/"

python3 - "build" "${name}" <<'PY'
import os, sys, zipfile
base, name = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(os.path.join(base, name + ".zip"), "w", zipfile.ZIP_DEFLATED) as z:
    for folder, _, files in os.walk(os.path.join(base, name)):
        for f in sorted(files):
            path = os.path.join(folder, f)
            z.write(path, os.path.relpath(path, base))
PY
(cd build && sha256sum "${name}.zip" > "${name}.zip.sha256")

echo "build/${name}.zip"
