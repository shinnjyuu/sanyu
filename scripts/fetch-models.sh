#!/bin/sh
set -eu

destination=${1:-models}
script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
registry_url=https://storage.googleapis.com/moz-fx-translations-data--303e-prod-translations-data

mkdir -p -- "$destination"
work=$(mktemp -d "$destination/.download-XXXXXX")
trap 'rm -rf -- "$work"' EXIT

while read -r digest filename path; do
    case "$digest" in '' | \#*) continue ;; esac
    file="$destination/$filename"
    if [ -f "$file" ] && printf '%s  %s\n' "$digest" "$file" | sha256sum --check --status; then
        continue
    fi

    printf 'Downloading %s\n' "$filename"
    curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' \
        --retry 3 --connect-timeout 15 --max-time 180 \
        --output "$work/archive.gz" "$registry_url/$path"
    gzip -dc "$work/archive.gz" > "$work/file"
    if ! printf '%s  %s\n' "$digest" "$work/file" | sha256sum --check --status; then
        printf 'SHA-256 mismatch for %s\n' "$filename" >&2
        exit 1
    fi
    mkdir -p -- "$(dirname -- "$file")"
    chmod 0444 "$work/file"
    mv -f -- "$work/file" "$file"
done < "$script_dir/firefox-models.txt"
