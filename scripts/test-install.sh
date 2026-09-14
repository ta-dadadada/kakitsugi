#!/bin/sh
set -eu

project_root="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
temp_root="$(mktemp -d "${TMPDIR:-/tmp}/kakitsugi-install-test.XXXXXX")"
cleanup() {
  rm -rf "$temp_root"
}
trap cleanup 0
trap 'exit 1' 1 2 15

case "$(uname -s):$(uname -m)" in
  Darwin:arm64 | Darwin:aarch64) target="aarch64-apple-darwin" ;;
  Linux:x86_64 | Linux:amd64) target="x86_64-unknown-linux-gnu" ;;
  *) printf '%s\n' "installer test requires a supported runner" >&2; exit 1 ;;
esac
asset="kakitsugi-${target}.tar.gz"
latest_dir="${temp_root}/releases/latest/download"
version_dir="${temp_root}/releases/download/v0.1.0"
invalid_dir="${temp_root}/releases/download/v0.1.1"
payload_dir="${temp_root}/payload"
install_dir="${temp_root}/bin with spaces"
fake_bin="${temp_root}/fake-bin"

mkdir -p "$latest_dir" "$version_dir" "$invalid_dir" "$payload_dir" "$fake_bin"
printf '#!/bin/sh\nprintf "fixture kakitsugi\\n"\n' > "${payload_dir}/kakitsugi"
chmod 0755 "${payload_dir}/kakitsugi"
printf '#!/bin/sh\nprintf "musl libc (x86_64)\\n"\n' > "${fake_bin}/getconf"
chmod 0755 "${fake_bin}/getconf"
tar -C "$payload_dir" -czf "${latest_dir}/${asset}" kakitsugi

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$latest_dir" && sha256sum "$asset" > "${asset}.sha256")
else
  (cd "$latest_dir" && shasum -a 256 "$asset" > "${asset}.sha256")
fi
cp "${latest_dir}/${asset}" "${version_dir}/${asset}"
cp "${latest_dir}/${asset}.sha256" "${version_dir}/${asset}.sha256"
cp "${latest_dir}/${asset}" "${invalid_dir}/${asset}"
cp "${latest_dir}/${asset}.sha256" "${invalid_dir}/${asset}.sha256"
printf '%s\n' "corrupted" >> "${invalid_dir}/${asset}"

KAKITSUGI_RELEASE_BASE_URL="file://${temp_root}/releases" \
KAKITSUGI_INSTALL_DIR="$install_dir" \
  sh "${project_root}/install.sh"

[ -x "${install_dir}/kakitsugi" ]
[ "$("${install_dir}/kakitsugi")" = "fixture kakitsugi" ]

rm -f "${install_dir}/kakitsugi"
KAKITSUGI_RELEASE_BASE_URL="file://${temp_root}/releases" \
KAKITSUGI_INSTALL_DIR="$install_dir" \
  sh "${project_root}/install.sh" 0.1.0

[ -x "${install_dir}/kakitsugi" ]
if KAKITSUGI_RELEASE_BASE_URL="file://${temp_root}/releases" \
  KAKITSUGI_INSTALL_DIR="$install_dir" \
  KAKITSUGI_TARGET="$target" \
  sh "${project_root}/install.sh" 0.1.1 >/dev/null 2>&1; then
  printf '%s\n' "checksum mismatch unexpectedly succeeded" >&2
  exit 1
fi
[ "$("${install_dir}/kakitsugi")" = "fixture kakitsugi" ]

if PATH="${fake_bin}:${PATH}" \
  KAKITSUGI_RELEASE_BASE_URL="file://${temp_root}/releases" \
  KAKITSUGI_INSTALL_DIR="$install_dir" \
  KAKITSUGI_TARGET="x86_64-unknown-linux-gnu" \
  sh "${project_root}/install.sh" >/dev/null 2>&1; then
  printf '%s\n' "musl platform unexpectedly succeeded" >&2
  exit 1
fi
[ "$("${install_dir}/kakitsugi")" = "fixture kakitsugi" ]

printf '#!/bin/sh\nprintf "glibc 2.38\\n"\n' > "${fake_bin}/getconf"
if PATH="${fake_bin}:${PATH}" \
  KAKITSUGI_RELEASE_BASE_URL="file://${temp_root}/releases" \
  KAKITSUGI_INSTALL_DIR="$install_dir" \
  KAKITSUGI_TARGET="x86_64-unknown-linux-gnu" \
  sh "${project_root}/install.sh" >/dev/null 2>&1; then
  printf '%s\n' "old glibc platform unexpectedly succeeded" >&2
  exit 1
fi
[ "$("${install_dir}/kakitsugi")" = "fixture kakitsugi" ]

if KAKITSUGI_TARGET="unsupported-target" sh "${project_root}/install.sh" >/dev/null 2>&1; then
  printf '%s\n' "unsupported target unexpectedly succeeded" >&2
  exit 1
fi

printf '%s\n' "installer smoke test passed"
