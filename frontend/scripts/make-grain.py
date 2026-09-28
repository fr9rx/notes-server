"""Generates src/assets/grain.png: a 160x160 tiling grayscale noise texture
(DESIGN.md §2.8). Deterministic, so re-running produces the same file.

    python scripts/make-grain.py
"""

import random
import struct
import zlib
from pathlib import Path

SIZE = 160
rng = random.Random(0x6E6F746573)  # "notes"

rows = b"".join(
    # filter byte + gray pixels in 5 levels (compresses far better than full-range noise)
    b"\x00" + bytes(64 + 32 * rng.randrange(5) for _ in range(SIZE))
    for _ in range(SIZE)
)


def chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)


png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 0, 0, 0, 0))  # 8-bit grayscale
    + chunk(b"IDAT", zlib.compress(rows, 9))
    + chunk(b"IEND", b"")
)
out = Path(__file__).resolve().parent.parent / "src" / "assets" / "grain.png"
out.parent.mkdir(parents=True, exist_ok=True)
out.write_bytes(png)
print(f"wrote {out} ({len(png)} bytes)")
