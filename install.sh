#!/bin/sh
# install.sh: seam from GitHub, its newest release, into ~/.local/share/seam
# (SEAM_DIR elsewhere; SEAM_VERSION=0.12.1 a release of your choosing), and a
# seam on your PATH in ~/.local/bin (SEAM_BIN elsewhere) that runs it there:
#
#   curl -fsSL https://raw.githubusercontent.com/booka66/seam/main/install.sh | sh
#
# Run again, it updates. seam finds share/ beside its own bin/, so the two
# stay together and what goes on PATH is a script that runs it, not a link.
# The release is unpacked beside the one there and swapped in; VERSION in it
# says which it is. The script on PATH is written again only when the
# release changes, since a cache may be keyed by its time (otis's are).
set -eu
repo=https://github.com/booka66/seam
dest=${SEAM_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/seam}
bin=${SEAM_BIN:-$HOME/.local/bin}

die() { printf 'seam: %s\n' "$1" >&2; exit 1; }
for c in curl tar; do command -v "$c" >/dev/null 2>&1 || die "needs $c"; done

# The newest release is the highest vX.Y.Z tag: tags alone, no GitHub
# release to publish. Asked of GitHub's API over curl, since a Mac that has
# never had the Command Line Tools has only a git that offers to install
# them; git only when the API says nothing (60 asks an hour an address).
newest() { sed -n "s|$1|\\1|p" | sort -t. -k1,1n -k2,2n -k3,3n | tail -1; }
version=${SEAM_VERSION:-$(curl -fsSL "https://api.github.com/repos/booka66/seam/tags?per_page=100" 2>/dev/null |
  newest '.*"name": *"v\([0-9]*\.[0-9]*\.[0-9]*\)".*')}
[ -n "$version" ] || version=$(git ls-remote --tags --refs "$repo" 'v*' 2>/dev/null |
  newest '.*refs/tags/v\([0-9]*\.[0-9]*\.[0-9]*\)$')
[ -n "$version" ] || die "could not find a release at $repo"

have=
if [ -e "$dest" ]; then
  [ -f "$dest/VERSION" ] || die "$dest is there and was not put there by this; move it, or set SEAM_DIR"
  have=$(cat "$dest/VERSION")
fi

if [ "$have" = "$version" ] && [ -x "$bin/seam" ]; then
  printf 'seam %s is the newest, and is in %s\n' "$version" "$dest"
else
  if [ "$have" != "$version" ]; then
    mkdir -p "$(dirname "$dest")"
    tmp=$(mktemp -d "$(dirname "$dest")/.seam.XXXXXX")
    trap 'rm -rf "$tmp"' EXIT
    curl -fsSL "$repo/archive/refs/tags/v$version.tar.gz" | tar -xzf - -C "$tmp" ||
      die "could not download seam $version"
    new=$tmp/seam-$version
    [ -x "$new/bin/seam" ] || die "seam $version did not unpack as expected"
    printf '%s\n' "$version" > "$new/VERSION"
    [ -e "$dest" ] && mv "$dest" "$tmp/old"
    mv "$new" "$dest"
    rm -rf "$tmp"
    trap - EXIT
  fi
  mkdir -p "$bin"
  printf '#!/bin/sh\nexec "%s/bin/seam" "$@"\n' "$dest" > "$bin/seam"
  chmod +x "$bin/seam"
  if [ -n "$have" ] && [ "$have" != "$version" ]; then printf 'seam %s -> %s, in %s\n' "$have" "$version" "$dest"
  else printf 'seam %s is in %s, and runs as %s/seam\n' "$version" "$dest" "$bin"; fi
fi

case ":$PATH:" in
  *":$bin:"*) ;;
  *) printf 'Put %s on your PATH to run it as seam.\n' "$bin" ;;
esac
for c in ast-grep jq; do
  command -v "$c" >/dev/null 2>&1 || printf 'seam needs %s too: brew install %s, or your package manager\n' "$c" "$c"
done
