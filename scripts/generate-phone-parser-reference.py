import argparse
import base64
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile

JAR_SHA256 = "c9efe6739636308930ecd2d3f282173e7d98a7c07592dbca466c02df8494f4a8"


def cases(path):
    with gzip.open(path, "rt") as source:
        header = json.loads(next(source))
        if header["reference"]["versions"]["libphonenumber"] != "9.0.41":
            raise ValueError("Expected the pinned Google 9.0.41 example corpus")
        examples = [json.loads(line) for line in source]
    result = {(case["region"], case["number"]) for case in examples}
    selected = {}
    for case in examples:
        number = case["number"]
        if case["expected"] and number.startswith("+") and number[1:].isascii() and number[1:].isdigit():
            selected.setdefault((case["region"], case["expected"]["numberType"]), number)
    letters = dict(zip("23456789", "ADGJMPTW"))
    for (region, _), number in selected.items():
        digits = number[1:]
        inputs = ["Call " + number, number + "#", number + "!", number[:4] + "!" + number[4:],
                  number[:4] + "\n" + number[4:], "tel:" + number + ";isub=99",
                  number[:6] + "".join(letters.get(ch, ch) for ch in number[6:])]
        for marker in [" ext. 42", "x42", "#42", ";ext=42", ",,42", ";42", "-42#", " anexo 42", " доб.42",
                       ";ext=" + "4" * 20, ";ext=" + "4" * 21, "x" + "4" * 9, "x" + "4" * 10]:
            inputs.append(number + marker)
        for zero in [0x660, 0x6F0, 0x966, 0xFF10, 0x1D7CE]:
            inputs.append("+" + "".join(chr(zero + int(ch)) for ch in digits))
            vanity = number[:6] + "".join(letters.get(ch, ch) for ch in number[6:])
            inputs.append("".join(chr(zero + int(ch)) if ch.isascii() and ch.isdigit() else ch for ch in vanity))
        result.update((region, value) for value in inputs)
        result.update([("", number), ("ZZ", number), ("US", "011" + digits), ("GB", "00" + digits), ("GB", "+00" + digits)])
    return sorted(result)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--classpath", required=True)
    parser.add_argument("--java-home", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    jars = [Path(value) for value in args.classpath.split(os.pathsep) if Path(value).name == "libphonenumber-9.0.41.jar"]
    if len(jars) != 1 or hashlib.sha256(jars[0].read_bytes()).hexdigest() != JAR_SHA256:
        raise ValueError("Expected the pinned Google libphonenumber 9.0.41 JAR")
    classpath = str(jars[0])
    root = Path(__file__).resolve().parents[1]
    example_path = root / "plain-rs/testdata/phone/reference.jsonl.gz"
    values = cases(example_path)
    data = "".join(region + "\t" + base64.b64encode(number.encode()).decode() + "\n" for region, number in values)
    with tempfile.TemporaryDirectory(prefix="plain-phone-parser-reference-") as directory:
        subprocess.run([str(args.java_home / "bin/javac"), "-cp", classpath, "-d", directory,
                        str(root / "scripts/PhoneNumberReference.java")], check=True)
        result = subprocess.run([str(args.java_home / "bin/java"), "-cp", directory + os.pathsep + classpath,
                                 "PhoneNumberReference"], input=data, text=True, capture_output=True, check=True)
    facts = [json.loads(line) for line in result.stdout.splitlines()]
    if len(facts) != len(values):
        raise ValueError("Google parser returned an incomplete replay")
    header = {"reference": {"source": "google-libphonenumber-public-examples-and-syntax-variants",
                             "normalization": "unicode-decimal-codepoints",
                             "version": "9.0.41", "case_count": len(values),
                             "jar_sha256": JAR_SHA256,
                             "examples_sha256": hashlib.sha256(example_path.read_bytes()).hexdigest()}}
    rows = [header] + [{"region": region, "number": number, "expected": expected}
                       for (region, number), expected in zip(values, facts)]
    raw = "".join(json.dumps(row, separators=(",", ":"), ensure_ascii=True) + "\n" for row in rows).encode()
    output = io.BytesIO()
    with gzip.GzipFile(fileobj=output, mode="wb", filename="", mtime=0) as writer:
        writer.write(raw)
    path = root / "plain-rs/testdata/phone/parser-reference.jsonl.gz"
    if args.check:
        if path.read_bytes() != output.getvalue():
            raise SystemExit("Google parser reference is stale")
    else:
        path.write_bytes(output.getvalue())
    print(f"{len(values)} offline Google parser cases, {path.stat().st_size} bytes")


if __name__ == "__main__":
    main()
