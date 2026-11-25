#!/usr/bin/env bash
set -euo pipefail

: "${VT_API_KEY:?Configure the VIRUSTOTAL_API_KEY secret before releasing}"
artifacts="${1:?Specify the release artifact directory}"
api=https://www.virustotal.com/api/v3

api_request() {
    curl --fail-with-body --silent --show-error --max-time 90 --header "x-apikey: $VT_API_KEY" "$@"
}

printf '### VirusTotal Scan\n| File | Status | Scan Report |\n|------|--------|-------------|\n'
for arch in amd64 arm64; do
    name="plainapp-linux-$arch.tar.gz"
    file="$artifacts/$name"
    digest=$(sha256sum "$file" | cut -d ' ' -f1)
    upload_url="$api/files"
    if [ "$(wc -c < "$file")" -gt 33554432 ]; then
        upload_url=$(api_request "$api/files/upload_url" | jq -er '.data')
        [[ "$upload_url" == http://www.virustotal.com/* ]] && upload_url="https://${upload_url#http://}"
        [[ "$upload_url" == https://www.virustotal.com/* ]]
    fi
    analysis=$(api_request --form "file=@$file" "$upload_url" | jq -er '.data.id | strings | select(length > 0) | @uri')
    response=
    for ((attempt = 0; attempt < 25; attempt++)); do
        sleep 20
        response=$(api_request "$api/analyses/$analysis")
        if jq -e '.data.attributes.status == "completed"' <<< "$response" >/dev/null; then
            break
        fi
    done
    read -r flagged total < <(jq -er '
        select(.data.attributes.status == "completed")
        | .data.attributes.stats
        | [(.malicious + .suspicious), (.malicious + .suspicious + .harmless + .undetected)]
        | @tsv' <<< "$response")
    test "$total" -gt 0
    if ((flagged == 0)); then
        printf -v status '%s %s/%s %s' '✅' "$flagged" "$total" 'Clean'
    else
        printf -v status '%s %s/%s %s' '⚠️' "$flagged" "$total" 'Detected'
    fi
    printf '| `%s` | %s | [View Report](https://www.virustotal.com/gui/file/%s/detection) |\n' "$name" "$status" "$digest"
done
