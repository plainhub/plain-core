import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const width = 1200
const height = 800
const header = Buffer.from(`P6\n${width} ${height}\n255\n`)
const pixels = Buffer.alloc(header.length + width * height * 3)
header.copy(pixels)
let offset = header.length
for (let y = 0; y < height; y++) {
  for (let x = 0; x < width; x++) {
    const textured = (Math.floor(x / 100) + Math.floor(y / 100)) % 5 === 0
    const texture = textured ? ((x * 13) ^ (y * 23)) % 64 : 0
    pixels[offset++] = (Math.floor(x / 5) + texture) % 256
    pixels[offset++] = (Math.floor(y / 3) + texture) % 256
    pixels[offset++] = (Math.floor((x + y) / 8) + texture) % 256
  }
}

const output = process.argv[2] ?? fileURLToPath(new URL('../plain-rs/testdata/v-synthetic-prog.jpg', import.meta.url))
execFileSync('magick', ['ppm:-', '-strip', '-sampling-factor', '2x2', '-interlace', 'Plane',
  '-quality', '88', output], { input: pixels })
