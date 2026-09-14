#!/bin/sh
set -eu

repository="ta-dadadada/kakitsugi"
release_base="${KAKITSUGI_RELEASE_BASE_URL:-https://github.com/${repository}/releases}"

fail() {
  printf 'kakitsugi installer: %s\n' "$1" >&2
  exit 1
}

usage() {
  printf '%s\n' \
    'Usage: install.sh [VERSION]' \
    '' \
    'Installs the latest Kakitsugi release by default.' \
    'VERSION may be v0.1.0 or 0.1.0.' \
    'Set KAKITSUGI_INSTALL_DIR to override the default ~/.local/bin.' \
    'KAKITSUGI_TARGET and KAKITSUGI_RELEASE_BASE_URL are advanced overrides.'
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
  exit 0
fi
[ "$#" -le 1 ] || fail "only one VERSION argument is accepted"

version="${1:-latest}"
if [ "$version" != "latest" ]; then
  case "$version" in
    v*) ;;
    [0-9]*) version="v${version}" ;;
    *) fail "VERSION must be latest or begin with a digit or v" ;;
  esac
  case "$version" in
    *[!0-9A-Za-z._+-]*) fail "VERSION contains unsupported characters" ;;
  esac
fi

if [ -n "${KAKITSUGI_INSTALL_DIR:-}" ]; then
  install_dir="$KAKITSUGI_INSTALL_DIR"
else
  [ -n "${HOME:-}" ] || fail "HOME is not set; set KAKITSUGI_INSTALL_DIR"
  install_dir="${HOME}/.local/bin"
fi

target="${KAKITSUGI_TARGET:-}"
if [ -z "$target" ]; then
  platform="$(uname -s):$(uname -m)"
  case "$platform" in
    Darwin:arm64 | Darwin:aarch64) target="aarch64-apple-darwin" ;;
    Linux:x86_64 | Linux:amd64) target="x86_64-unknown-linux-gnu" ;;
    *) fail "unsupported platform ${platform}; use a supported release or build from source" ;;
  esac
fi

case "$target" in
  aarch64-apple-darwin | x86_64-unknown-linux-gnu) ;;
  *) fail "unsupported target ${target}" ;;
esac

if [ "$target" = "x86_64-unknown-linux-gnu" ]; then
  command -v getconf >/dev/null 2>&1 ||
    fail "Linux releases require glibc 2.39 or later; getconf was not found"
  libc_version="$(getconf GNU_LIBC_VERSION 2>/dev/null)" ||
    fail "Linux releases require glibc 2.39 or later"
  case "$libc_version" in
    "glibc "*) ;;
    *) fail "Linux releases require glibc 2.39 or later; found ${libc_version}" ;;
  esac

  libc_number="${libc_version#glibc }"
  libc_major="${libc_number%%.*}"
  libc_minor_rest="${libc_number#*.}"
  libc_minor="${libc_minor_rest%%.*}"
  case "${libc_major}:${libc_minor}" in
    *[!0-9:]* | :* | *:) fail "could not determine the installed glibc version" ;;
  esac
  if [ "$libc_major" -lt 2 ] || {
    [ "$libc_major" -eq 2 ] && [ "$libc_minor" -lt 39 ]
  }; then
    fail "Linux releases require glibc 2.39 or later; found ${libc_number}"
  fi
fi

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"

asset="kakitsugi-${target}.tar.gz"
if [ "$version" = "latest" ]; then
  download_base="${release_base%/}/latest/download"
else
  download_base="${release_base%/}/download/${version}"
fi

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/kakitsugi.XXXXXX")"
staged_binary=""
cleanup() {
  rm -rf "$temp_dir"
  if [ -n "$staged_binary" ]; then
    rm -f "$staged_binary"
  fi
}
trap cleanup 0
trap 'exit 1' 1 2 15

curl --proto '=https,file' --proto-redir '=https' --fail --location --silent --show-error \
  --output "${temp_dir}/${asset}" "${download_base}/${asset}"
curl --proto '=https,file' --proto-redir '=https' --fail --location --silent --show-error \
  --output "${temp_dir}/${asset}.sha256" "${download_base}/${asset}.sha256"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$temp_dir" && sha256sum -c "${asset}.sha256")
elif command -v shasum >/dev/null 2>&1; then
  (cd "$temp_dir" && shasum -a 256 -c "${asset}.sha256")
else
  fail "sha256sum or shasum is required"
fi

tar -xzf "${temp_dir}/${asset}" -C "$temp_dir"
[ -f "${temp_dir}/kakitsugi" ] || fail "release archive does not contain kakitsugi"

mkdir -p "$install_dir"
staged_binary="$(mktemp "${install_dir}/.kakitsugi.XXXXXX")"
if command -v install >/dev/null 2>&1; then
  install -m 0755 "${temp_dir}/kakitsugi" "$staged_binary"
else
  cp "${temp_dir}/kakitsugi" "$staged_binary"
  chmod 0755 "$staged_binary"
fi
mv -f "$staged_binary" "${install_dir}/kakitsugi"
staged_binary=""

printf 'Installed kakitsugi to %s\n' "${install_dir}/kakitsugi"
case ":${PATH}:" in
  *":${install_dir}:"*) ;;
  *) printf 'Add %s to PATH before running kakitsugi.\n' "$install_dir" ;;
esac
