#!/bin/sh
# Point Formula/mynalist.rb's stable release at a pushed tag (url + sha256), so that
# `brew install mynalist` works without --HEAD. Usage, after `git push origin <tag>`:
#   deploy/homebrew/bump.sh v0.1.0
# Then commit Formula/mynalist.rb. Users get it with `brew update && brew upgrade mynalist`.
set -eu
tag=${1:?usage: bump.sh <tag>}
cd "$(dirname "$0")/../.."
url="https://github.com/jadewisemann/mynalist/archive/refs/tags/$tag.tar.gz"
sha=$(curl -fsSL "$url" | shasum -a 256 | cut -d' ' -f1)
f=Formula/mynalist.rb
# Drop any previous stable url/sha256, then add the new ones under homepage.
grep -v -E '^  (url "https://github.com/jadewisemann/mynalist/archive/|sha256 ")' "$f" |
  awk -v url="$url" -v sha="$sha" '{ print } /^  homepage / { print "  url \"" url "\""; print "  sha256 \"" sha "\"" }' > "$f.tmp"
mv "$f.tmp" "$f"
echo "$f → $tag ($sha)"
