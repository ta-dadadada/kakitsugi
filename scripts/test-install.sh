#!/bin/sh
set -eu

project_root="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
temp_root="$(mktemp -d "${TMPDIR:-/tmp}/kakitsugi-install-test.XXXXXX")"
cleanup() {
  rm -rf "$temp_root"
}
trap cleanup 0
trap 'exit 1' 1 2 15

targets="aarch64-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl"
case "$(uname -s):$(uname -m)" in
  Darwin:arm64 | Darwin:aarch64) target="aarch64-apple-darwin" ;;
  Linux:x86_64 | Linux:amd64) target="x86_64-unknown-linux-musl" ;;
  Linux:aarch64 | Linux:arm64) target="aarch64-unknown-linux-musl" ;;
  *) printf '%s\n' "installer test requires a supported runner" >&2; exit 1 ;;
esac
latest_dir="${temp_root}/releases/latest/download"
version_dir="${temp_root}/releases/download/v0.1.0"
invalid_dir="${temp_root}/releases/download/v0.1.1"
payload_root="${temp_root}/payload"
install_dir="${temp_root}/bin with spaces"
fake_bin="${temp_root}/fake-bin"

checksum() {
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$1" && sha256sum "$2" > "$2.sha256")
  else
    (cd "$1" && shasum -a 256 "$2" > "$2.sha256")
  fi
}

mkdir -p "$latest_dir" "$version_dir" "$invalid_dir" "$fake_bin"
for fixture_target in $targets; do
  payload_dir="${payload_root}/${fixture_target}"
  fixture_asset="kakitsugi-${fixture_target}.tar.gz"
  mkdir -p "$payload_dir"
  printf '#!/bin/sh\nprintf "fixture kakitsugi %s\\n"\n' "$fixture_target" > "${payload_dir}/kakitsugi"
  chmod 0755 "${payload_dir}/kakitsugi"
  tar -C "$payload_dir" -czf "${latest_dir}/${fixture_asset}" kakitsugi
  checksum "$latest_dir" "$fixture_asset"
done

asset="kakitsugi-${target}.tar.gz"
cp "${latest_dir}/${asset}" "${version_dir}/${asset}"
cp "${latest_dir}/${asset}.sha256" "${version_dir}/${asset}.sha256"
cp "${latest_dir}/${asset}" "${invalid_dir}/${asset}"
cp "${latest_dir}/${asset}.sha256" "${invalid_dir}/${asset}.sha256"
printf '%s\n' "corrupted" >> "${invalid_dir}/${asset}"

KAKITSUGI_RELEASE_BASE_URL="file://${temp_root}/releases" \
KAKITSUGI_INSTALL_DIR="$install_dir" \
  sh "${project_root}/install.sh"

[ -x "${install_dir}/kakitsugi" ]
[ "$("${install_dir}/kakitsugi")" = "fixture kakitsugi ${target}" ]

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
[ "$("${install_dir}/kakitsugi")" = "fixture kakitsugi ${target}" ]

# uname の結果から対応するリリース資産を選ぶことを確認する。
for platform in Darwin:arm64 Darwin:aarch64 Linux:x86_64 Linux:amd64 Linux:aarch64 Linux:arm64; do
  case "$platform" in
    Darwin:*) expected="aarch64-apple-darwin" ;;
    Linux:x86_64 | Linux:amd64) expected="x86_64-unknown-linux-musl" ;;
    Linux:*) expected="aarch64-unknown-linux-musl" ;;
  esac
  printf '#!/bin/sh\ncase "$1" in -s) printf "%%s\\n" "%s" ;; -m) printf "%%s\\n" "%s" ;; esac\n' \
    "${platform%%:*}" "${platform#*:}" > "${fake_bin}/uname"
  chmod 0755 "${fake_bin}/uname"
  PATH="${fake_bin}:${PATH}" \
    KAKITSUGI_RELEASE_BASE_URL="file://${temp_root}/releases" \
    KAKITSUGI_INSTALL_DIR="$install_dir" \
    sh "${project_root}/install.sh" >/dev/null
  [ "$("${install_dir}/kakitsugi")" = "fixture kakitsugi ${expected}" ] || {
    printf '%s\n' "platform ${platform} did not install ${expected}" >&2
    exit 1
  }
done

for explicit_target in x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
  KAKITSUGI_RELEASE_BASE_URL="file://${temp_root}/releases" \
    KAKITSUGI_INSTALL_DIR="$install_dir" \
    KAKITSUGI_TARGET="$explicit_target" \
    sh "${project_root}/install.sh" >/dev/null
  [ "$("${install_dir}/kakitsugi")" = "fixture kakitsugi ${explicit_target}" ]
done

printf '#!/bin/sh\ncase "$1" in -s) printf "Linux\\n" ;; -m) printf "riscv64\\n" ;; esac\n' \
  > "${fake_bin}/uname"
if PATH="${fake_bin}:${PATH}" \
  KAKITSUGI_RELEASE_BASE_URL="file://${temp_root}/releases" \
  KAKITSUGI_INSTALL_DIR="$install_dir" \
  sh "${project_root}/install.sh" >/dev/null 2>&1; then
  printf '%s\n' "unsupported platform unexpectedly succeeded" >&2
  exit 1
fi

# 旧版の glibc 向け target は配布しない。
if KAKITSUGI_TARGET="x86_64-unknown-linux-gnu" sh "${project_root}/install.sh" >/dev/null 2>&1; then
  printf '%s\n' "retired gnu target unexpectedly succeeded" >&2
  exit 1
fi

if KAKITSUGI_TARGET="unsupported-target" sh "${project_root}/install.sh" >/dev/null 2>&1; then
  printf '%s\n' "unsupported target unexpectedly succeeded" >&2
  exit 1
fi

printf '%s\n' "installer smoke test passed"
