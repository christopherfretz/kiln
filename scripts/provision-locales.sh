#!/usr/bin/env bash
# Generate the locales libc++'s test suite probes for, so `cargo xtask
# libcxx` gets the `locale.*` lit features (spec/libcxx.md "Feature set").
#
# libc++'s lit config (libcxx/utils/libcxx/test/features.py `locales`) adds
# `locale.<name>` when setlocale() accepts <name>; xtask libcxx reads
# `locale -a` instead and normalises the codeset the way glibc does
# (`utf8` -> `UTF-8`, `iso88592` -> `ISO8859-2`). The two ISO-8859 locales
# are generated under a name that carries the codeset (`cs_CZ.ISO-8859-2`),
# because a bare `cs_CZ` would not show up as `cs_CZ.iso88592`.
#
# Debian/Ubuntu only (apt-get, /etc/locale.gen, locale-gen); needs root.
# Idempotent and quiet: exits 0 without touching anything when every locale
# is already present (a few ms). Installs the `locales` package only when
# locale-gen is missing.
#
# The generated locales live in the container's /usr/lib/locale, not in the
# workspace, so an ephemeral container loses them on every restart: run this
# at each container start, before a libc++ run (a fresh container takes
# about 10 s). Waits for the dpkg lock when another apt-get is running.
#
# Usage: scripts/provision-locales.sh
set -euo pipefail

# <locale.gen name> <charset> <`locale -a` name it yields>
LOCALES=(
  "en_US.UTF-8 UTF-8 en_US.utf8"
  "fr_FR.UTF-8 UTF-8 fr_FR.utf8"
  "ja_JP.UTF-8 UTF-8 ja_JP.utf8"
  "ru_RU.UTF-8 UTF-8 ru_RU.utf8"
  "zh_CN.UTF-8 UTF-8 zh_CN.utf8"
  "cs_CZ.ISO-8859-2 ISO-8859-2 cs_CZ.iso88592"
  "fr_CA.ISO-8859-1 ISO-8859-1 fr_CA.iso88591"
)

missing() {
  local have entry
  have="$(locale -a 2>/dev/null || true)"
  for entry in "${LOCALES[@]}"; do
    read -r _ _ name <<<"$entry"
    grep -qxF "$name" <<<"$have" || return 0
  done
  return 1
}

missing || exit 0

if [ "$(id -u)" != 0 ]; then
  echo "provision-locales: locales missing and not root; run as root" >&2
  exit 1
fi

# locale-gen lives in /usr/sbin, which a container's PATH may lack.
export PATH="$PATH:/usr/sbin:/sbin"
if ! command -v locale-gen >/dev/null 2>&1; then
  export DEBIAN_FRONTEND=noninteractive
  apt=(apt-get -o DPkg::Lock::Timeout=600)
  if ! "${apt[@]}" install -y -qq locales >/dev/null 2>&1; then
    "${apt[@]}" update -qq >/dev/null
    "${apt[@]}" install -y -qq locales >/dev/null
  fi
fi

touch /etc/locale.gen
for entry in "${LOCALES[@]}"; do
  read -r gen charset _ <<<"$entry"
  grep -qxF "$gen $charset" /etc/locale.gen || echo "$gen $charset" >>/etc/locale.gen
done
locale-gen >/dev/null

if missing; then
  echo "provision-locales: locale-gen ran but some locales are still missing:" >&2
  locale -a >&2
  exit 1
fi
