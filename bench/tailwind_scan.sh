#!/bin/sh
# Tailwind @source scanning verification.
#
# Question: does Tailwind v4's class scanner extract the same utility set
# from .fhtml sources as from the equivalent .html — including arbitrary
# values (p-[3px]), variants (hover:, data-[state=open]:), and fractions
# (w-1/2)?
#
# Method: build CSS twice from the same 48-component corpus — once with
# @source pointed at the HTML originals, once at their html2fhtml
# conversions (source(none) disables auto-detection, isolating the test).
# Identical CSS ⇒ identical candidate extraction.
#
# Setup (once): npm install --prefix bench/.tools tailwindcss @tailwindcss/cli
set -eu

cd "$(dirname "$0")"
TW=.tools/node_modules/.bin/tailwindcss
H2F=../target/release/html2fhtml
WORK=out/twscan

[ -x "$TW" ] || { echo "tailwind CLI missing — see setup line in this script"; exit 1; }
[ -x "$H2F" ] || { echo "$H2F missing — cargo build --release --features convert"; exit 1; }

rm -rf "$WORK"
mkdir -p "$WORK/html" "$WORK/fhtml"
cp corpus/*.html "$WORK/html/"
"$H2F" corpus -o "$WORK/fhtml"

# node resolution won't find bench/.tools/node_modules from out/twscan,
# so import the package CSS by relative path.
for src in html fhtml; do
  printf '@import "../../.tools/node_modules/tailwindcss/index.css" source(none);\n@source "./%s";\n' "$src" \
    > "$WORK/in-$src.css"
  "$TW" -i "$WORK/in-$src.css" -o "$WORK/out-$src.css"
done

# Lines only in out-html.css (`<`) are classes the fhtml scan MISSED — the
# failure mode this test exists for. Lines only in out-fhtml.css (`>`) are
# extras: bare fhtml tag tokens that happen to name a utility (`table` →
# `.table{display:table}`) — harmless dead CSS, reported but not fatal.
missed=$(diff "$WORK/out-html.css" "$WORK/out-fhtml.css" | grep '^<' || true)
extra=$(diff "$WORK/out-html.css" "$WORK/out-fhtml.css" | grep '^>' || true)

if [ -n "$missed" ]; then
  echo "FAIL: classes extracted from .html but NOT from .fhtml:"
  echo "$missed" | head -40
  exit 1
fi
size=$(wc -c < "$WORK/out-fhtml.css" | tr -d ' ')
echo "PASS: .fhtml scan covers every class the .html scan found"
echo "      ($size bytes of CSS, tailwindcss $("$TW" --help 2>&1 | head -1 | awk '{print $NF}'))"
if [ -n "$extra" ]; then
  echo "note: superset — extra utilities from bare tag tokens (harmless):"
  echo "$extra" | grep '{' | tr -d '>{ '
fi
