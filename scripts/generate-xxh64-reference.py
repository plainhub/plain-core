import argparse
import ctypes
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--header", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    header = args.header.read_bytes()
    # Official xxHash v0.8.3 xxhash.h; no reference code is compiled into the application.
    digest = "17973c0dc49d9854ca26caa191f0e12f7a424b68858d9a78de3860d959d85e4b"
    if hashlib.sha256(header).hexdigest() != digest:
        raise ValueError("xxHash v0.8.3 header checksum mismatch")
    with tempfile.TemporaryDirectory() as temporary:
        directory = Path(temporary)
        (directory / "xxhash.h").write_bytes(header)
        source = directory / "reference.c"
        source.write_text('#define XXH_STATIC_LINKING_ONLY\n#define XXH_IMPLEMENTATION\n#include "xxhash.h"\n')
        library = directory / ("reference.dylib" if sys.platform == "darwin" else "reference.so")
        subprocess.run(["cc", "-O2", "-dynamiclib" if sys.platform == "darwin" else "-shared", "-fPIC", str(source), "-o", str(library)], check=True)
        reference = ctypes.CDLL(str(library)).XXH64
        reference.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_uint64]
        reference.restype = ctypes.c_uint64
        lengths = list(range(1025)) + [2047, 2048, 2049, 4095, 4096, 4097, 65535, 65536, 65537]
        result = bytearray()
        for pattern in range(4):
            for length in lengths:
                data = bytes(0 if pattern == 0 else 255 if pattern == 1 else index % 256 if pattern == 2 else (index * 131 + 17) % 256 for index in range(length))
                result.extend(reference(data, len(data), 0).to_bytes(8, "little"))
    output = Path(__file__).resolve().parents[1] / "plain-rs/testdata/xxh64/reference.bin"
    if args.check:
        if output.read_bytes() != result:
            raise ValueError("XXH64 reference fixture is stale")
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(result)


if __name__ == "__main__":
    main()
