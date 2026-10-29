import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import urllib.request
import xml.etree.ElementTree as ET

METADATA_URL = "https://raw.githubusercontent.com/google/libphonenumber/b68df884d42d5e9860fc1956a99ce6afae361100/resources/PhoneNumberMetadata.xml"
METADATA_SHA256 = "9d3b07b53c7e8a437ddc316249620b51d4090987026f9e8ffb9743aab4e4d5a6"
UNICODE_URL = "https://www.unicode.org/Public/16.0.0/ucd/UnicodeData.txt"
UNICODE_SHA256 = "ff58e5823bd095166564a006e47d111130813dcf8bf234ef79fa51a870edb48f"
KINDS = {
    "fixed_line": "fixedLine", "mobile": "mobile", "toll_free": "tollFree",
    "premium_rate": "premiumRate", "shared_cost": "sharedCost", "personal_number": "personalNumber",
    "voip": "voip", "pager": "pager", "uan": "uan", "voicemail": "voicemail",
}


def source(path, url, digest):
    raw = path.read_bytes() if path else urllib.request.urlopen(url, timeout=30).read()
    if hashlib.sha256(raw).hexdigest() != digest:
        raise ValueError("Pinned metadata source checksum mismatch: " + url)
    return raw


def expression(value, full=True):
    value = re.sub(r"\s+", "", value or "")
    return "^(?:" + value + ")" + ("$" if full else "") if value else ""


def lengths(value):
    result = []
    for item in value.split(","):
        if not item:
            continue
        if item.startswith("[") and item.endswith("]"):
            first, last = map(int, item[1:-1].split("-"))
            result.extend(range(first, last + 1))
        else:
            result.append(int(item))
    return sorted(set(result))


def description(territory, name):
    node = territory.find(name)
    if node is None:
        return {"pattern": "", "lengths": []}
    possible = node.find("possibleLengths")
    return {
        "pattern": expression(node.findtext("nationalNumberPattern")),
        "lengths": lengths(possible.attrib["national"]) if possible is not None else [],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--metadata", type=Path)
    parser.add_argument("--unicode", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = ET.fromstring(source(args.metadata, METADATA_URL, METADATA_SHA256))
    territories = sorted(root.findall("territories/territory"), key=lambda t: (
        int(t.attrib["countryCode"]), t.attrib.get("mainCountryForCode") != "true", t.attrib["id"]))
    plans = []
    for territory in territories:
        item = {
            "region": territory.attrib["id"], "country_code": int(territory.attrib["countryCode"]),
            "international_prefix": expression(territory.attrib.get("internationalPrefix"), False),
            "national_prefix": expression(territory.attrib.get("nationalPrefixForParsing", territory.attrib.get("nationalPrefix")), False),
            "transform": re.sub(r"\$(\d)", r"${\1}", territory.attrib.get("nationalPrefixTransformRule", "")),
            "leading_digits": expression(territory.attrib.get("leadingDigits"), False),
            "general": expression(territory.findtext("generalDesc/nationalNumberPattern")),
        }
        for field, tag in KINDS.items():
            item[field] = description(territory, tag)
        item["lengths"] = sorted({length for field in KINDS for length in item[field]["lengths"]})
        if not item["general"] or not item["lengths"]:
            raise ValueError("Incomplete numbering plan: " + item["region"])
        plans.append(item)
    decimal = {}
    for line in source(args.unicode, UNICODE_URL, UNICODE_SHA256).decode().splitlines():
        fields = line.split(";")
        if fields[6]:
            decimal[int(fields[0], 16)] = int(fields[6])
    zeroes = sorted(point for point, value in decimal.items() if value == 0)
    for point in zeroes:
        if any(decimal.get(point + digit) != digit for digit in range(10)):
            raise ValueError("Non-contiguous Unicode decimal digits")
    raw = json.dumps({"plans": plans, "decimal_zeroes": zeroes}, separators=(",", ":"), ensure_ascii=True).encode()
    output = io.BytesIO()
    with gzip.GzipFile(fileobj=output, mode="wb", filename="", mtime=0) as writer:
        writer.write(raw)
    path = Path(__file__).resolve().parents[1] / "plain-rs/src/phone/data/numbering.json.gz"
    if args.check:
        if path.read_bytes() != output.getvalue():
            raise SystemExit("Bundled numbering data is stale")
    else:
        path.write_bytes(output.getvalue())
    print(f"{len(plans)} numbering plans, {len(zeroes)} decimal alphabets, {len(output.getvalue())} bytes")


if __name__ == "__main__":
    main()
