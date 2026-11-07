#!/usr/bin/env python3
from pathlib import Path
import subprocess


def main():
    width, height = 1200, 800
    pixels = bytearray(f"P6\n{width} {height}\n255\n".encode("ascii"))
    for y in range(height):
        for x in range(width):
            texture = ((x * 13) ^ (y * 23)) % 64 if (x // 100 + y // 100) % 5 == 0 else 0
            pixels.extend(((x // 5 + texture) % 256, (y // 3 + texture) % 256, ((x + y) // 8 + texture) % 256))

    output = Path(__file__).resolve().parents[1] / "plain-rs/testdata/v-synthetic-prog.jpg"
    subprocess.run(
        ["magick", "ppm:-", "-strip", "-sampling-factor", "2x2", "-interlace", "Plane",
         "-quality", "88", str(output)],
        input=pixels,
        check=True,
    )


if __name__ == "__main__":
    main()
