#!/bin/sh
set -eu

project_root="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
packslip_bin="$(command -v "${PACKSLIP_BIN:-packslip}")" || {
  printf '%s\n' "packslip 1.2.0 is required" >&2
  exit 1
}

if [ "$("$packslip_bin" version)" != "packslip 1.2.0" ]; then
  printf '%s\n' "packslip 1.2.0 is required" >&2
  exit 1
fi

temp_root="$(mktemp -d "${TMPDIR:-/tmp}/kakitsugi-packslip-test.XXXXXX")"
cleanup() {
  rm -rf "$temp_root"
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$temp_root/dist" "$temp_root/payload" "$temp_root/skills"
cp "$project_root/packslip.toml" "$temp_root/packslip.toml"
cp -R "$project_root/skills/kakitsugi" "$temp_root/skills/kakitsugi"
printf '#!/bin/sh\nprintf "kakitsugi fixture\\n"\n' > "$temp_root/payload/kakitsugi"
chmod 0755 "$temp_root/payload/kakitsugi"
tar -C "$temp_root/payload" -czf \
  "$temp_root/dist/kakitsugi-aarch64-apple-darwin.tar.gz" kakitsugi
tar -C "$temp_root/payload" -czf \
  "$temp_root/dist/kakitsugi-x86_64-unknown-linux-gnu.tar.gz" kakitsugi

(
  cd "$temp_root"
  git init -q
  git config user.name "Kakitsugi distribution test"
  git config user.email "distribution-test@example.invalid"
  git config commit.gpgSign false
  git config core.hooksPath /dev/null
  git add packslip.toml skills
  git commit -qm "Add distribution fixture"
  commit="$(git rev-parse HEAD)"

  "$packslip_bin" keygen --out release.key
  "$packslip_bin" create \
    --project github.com/ta-dadadada/kakitsugi \
    --version 0.1.0 \
    --manifest packslip.toml \
    --source-repo https://github.com/ta-dadadada/kakitsugi \
    --commit "$commit" \
    --tag v0.1.0 \
    --url-base https://github.com/ta-dadadada/kakitsugi/releases/download/v0.1.0 \
    --key release.key \
    --no-log \
    --no-libs \
    --out output
  "$packslip_bin" show output/packslip.sigstore.json >/dev/null
  "$packslip_bin" verify output/packslip.sigstore.json \
    --pubkey release.pub \
    --allow-unlogged \
    --artifact dist/kakitsugi-aarch64-apple-darwin.tar.gz \
    --artifact dist/kakitsugi-x86_64-unknown-linux-gnu.tar.gz
)

printf '%s\n' "packslip contract test passed"
