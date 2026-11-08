#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

sdk_repository="${SDK_REPOSITORY:-l0ng-ai/openidea}"
sdk_ref="${SDK_REF:?set SDK_REF to the OpenIDEA release whose openidea-plugin CLI packs the plugin}"
publisher="pedro"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' plugin.toml | head -n 1)"
id="$(sed -n 's/^id = "\(.*\)"/\1/p' plugin.toml | head -n 1)"
folder="build/laravel-openidea-${version}"
package="build/${id}-${version}.oiplugin"

test -d "${folder}" || { echo "${folder} is missing: run scripts/package.sh first"; exit 1; }

cli="${OPENIDEA_PLUGIN_CLI:-}"
if [ -z "${cli}" ]; then
  cli="build/tools/openidea-plugin"
  mkdir -p build/tools
  curl -fsSL "https://github.com/${sdk_repository}/releases/download/${sdk_ref}/openidea-plugin-linux-x86_64.gz" | gunzip > "${cli}"
  chmod +x "${cli}"
fi

if [ -n "${OPENIDEA_PUBLISHER_KEY:-}" ]; then
  key_file="$(mktemp)"
  trap 'rm -f "${key_file}"' EXIT
  printf '%s' "${OPENIDEA_PUBLISHER_KEY}" > "${key_file}"
  "${cli}" pack "${folder}" --key "${key_file}" --publisher "${publisher}" --out "${package}"
else
  "${cli}" pack "${folder}" --out "${package}"
fi

"${cli}" check "${package}"
(cd build && sha256sum "$(basename "${package}")" > "$(basename "${package}").sha256")

echo "${package}"
