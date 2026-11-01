import argparse
import re
import struct
from pathlib import Path


def array(text, name):
    match = re.search(r"\b" + name + r"\s*=\s*byteArrayOf\((.*?)\)", text, re.S)
    if match is None:
        raise ValueError(f"Missing character table: {name}")
    return [int(value) & 255 for value in re.findall(r"-?\d+", match.group(1))]


def main():
    parser = argparse.ArgumentParser(
        description="Import PlainApp's default plain-common Kotlin pinyin character tables."
    )
    parser.add_argument("source", type=Path, help="Directory containing PinyinData.kt and PinyinCode1/2/3.kt")
    args = parser.parse_args()
    text = (args.source / "PinyinData.kt").read_text()
    syllables = re.findall(r'"([^"\n]*)"', text[text.index("arrayOf("):])
    if not syllables or syllables[0] != "":
        raise ValueError("Invalid syllable table")
    indices = []
    for part in (1, 2, 3):
        text = (args.source / f"PinyinCode{part}.kt").read_text()
        padding = array(text, "PINYIN_CODE_PADDING")
        values = array(text, "PINYIN_CODE")
        if len(values) != 7000 or len(padding) != 875:
            raise ValueError("Invalid character table size")
        indices.extend(
            value | (256 if padding[index // 8] & (1 << (index % 8)) else 0)
            for index, value in enumerate(values)
        )
    indices = indices[:0x9FA5 - 0x4E00 + 1]
    if max(indices) >= len(syllables):
        raise ValueError("Invalid character table index")
    target = Path(__file__).resolve().parent.parent / "plain-libs/src/utils/sort_name"
    target.mkdir(parents=True, exist_ok=True)
    (target / "characters.bin").write_bytes(b"".join(struct.pack("<H", value) for value in indices))
    rows = [
        "    " + ", ".join('"' + word + '"' for word in syllables[index:index + 10]) + ",\n"
        for index in range(0, len(syllables), 10)
    ]
    (target / "syllables.rs").write_text("pub(super) const SYLLABLES: &[&str] = &[\n" + "".join(rows) + "];\n")


if __name__ == "__main__":
    main()
