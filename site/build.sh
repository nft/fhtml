#!/bin/sh
# Build the fhtml.dev one-pager into dist/.
#
#   fhtml (--min --data) ──▶ dist/index.html
#   tailwindcss           ──▶ dist/site.css   (scans site/index.fhtml directly)
#   cp static             ──▶ dist/static/
#
# Runnable from anywhere — it cd's to the repo root first. CI and local dev
# find the Tailwind binary differently, so it is resolved via $TAILWIND_BIN
# (default ./tailwindcss), then the bench toolchain, then $PATH.
set -eu

cd "$(dirname "$0")/.."

FHTML_BIN="${FHTML_BIN:-cargo run --release --bin fhtml --}"
OUT=dist

# Resolve a Tailwind CLI. input.css uses a bare `@import "tailwindcss"`, which the
# self-contained standalone binary bundles — so that's the target (also what CI uses
# and what keeps the repo Node-free). The bench node install is NOT usable here: node
# module resolution from site/ can't reach bench/.tools/node_modules.
if [ -n "${TAILWIND_BIN:-}" ]; then
  TW="$TAILWIND_BIN"
elif [ -x ./tailwindcss ]; then
  TW=./tailwindcss
elif command -v tailwindcss >/dev/null 2>&1; then
  TW=tailwindcss
else
  plat=$(uname -s | tr '[:upper:]' '[:lower:]'); arch=$(uname -m)
  case "$plat-$arch" in
    darwin-arm64) asset=tailwindcss-macos-arm64 ;;
    darwin-x86_64) asset=tailwindcss-macos-x64 ;;
    linux-x86_64|linux-amd64) asset=tailwindcss-linux-x64 ;;
    linux-aarch64|linux-arm64) asset=tailwindcss-linux-arm64 ;;
    *) asset='tailwindcss-<your-platform>' ;;
  esac
  echo "error: no tailwindcss binary found." >&2
  echo "  Download the standalone CLI once (repo stays Node-free):" >&2
  echo "    curl -fsSLo tailwindcss https://github.com/tailwindlabs/tailwindcss/releases/latest/download/$asset && chmod +x tailwindcss" >&2
  echo "  or set TAILWIND_BIN to an existing tailwindcss v4 binary." >&2
  exit 1
fi

# Guard: the site serves from a /fhtml/ subpath, so every asset reference must
# be relative. A root-absolute href/src would 404 under the subpath. Real fhtml
# asset attributes are unquoted barewords (href=site.css); this page also *displays*
# markup samples containing `href=/…`, but those always sit inside `|` text blocks
# or "quoted strings" — both stripped here so only genuine attributes are scanned.
# (og:/canonical URLs are absolute-by-design, built from site_url, and quoted.)
if grep -vE '^[[:space:]]*\|' site/index.fhtml | sed 's/"[^"]*"//g' | grep -nE '(href|src)=/'; then
  echo "error: root-absolute asset path in site/index.fhtml (see match above)" >&2
  echo "       use a relative path so the page works under the /fhtml/ subpath." >&2
  exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT"

echo "· fhtml  → $OUT/index.html"
$FHTML_BIN build site/ -o "$OUT" --min --data site/data.json

echo "· tailwind → $OUT/site.css   ($TW)"
"$TW" -i site/input.css -o "$OUT/site.css" --minify

echo "· static → $OUT/static/"
cp -R site/static "$OUT/static"

echo "done → $OUT/ ($(wc -c < "$OUT/index.html" | tr -d ' ') B html, $(wc -c < "$OUT/site.css" | tr -d ' ') B css)"
