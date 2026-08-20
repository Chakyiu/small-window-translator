#!/usr/bin/env python3
"""Write the bilingual dictionary mark as PNG and Windows ICO icons."""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "resources" / "icons"

COVER = (0x0A, 0x84, 0xFF, 255)
COVER_DARK = (0x00, 0x55, 0xB8, 255)
PAGE = (0xF5, 0xF5, 0xF7, 255)
RULE = (0xC7, 0xC7, 0xCC, 255)
INK = (0x1C, 0x1C, 0x1E, 255)
RIBBON = (0xFF, 0x45, 0x3A, 255)
CLEAR = (0, 0, 0, 0)


def put(px: list[list[tuple[int, int, int, int]]], x: int, y: int, c: tuple[int, int, int, int]) -> None:
    h = len(px)
    w = len(px[0])
    if 0 <= x < w and 0 <= y < h:
        px[y][x] = c


def fill(
    px: list[list[tuple[int, int, int, int]]],
    x0: int,
    y0: int,
    x1: int,
    y1: int,
    c: tuple[int, int, int, int],
) -> None:
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            put(px, x, y, c)


def draw(size: int) -> list[list[tuple[int, int, int, int]]]:
    s = size / 64.0
    sc = lambda n: int(round(n * s))
    px = [[CLEAR for _ in range(size)] for _ in range(size)]

    fill(px, sc(8), sc(10), sc(55), sc(52), COVER)
    fill(px, sc(10), sc(8), sc(53), sc(9), COVER)
    fill(px, sc(10), sc(53), sc(53), sc(54), COVER)
    fill(px, sc(12), sc(14), sc(30), sc(46), PAGE)
    fill(px, sc(33), sc(14), sc(51), sc(46), PAGE)
    fill(px, sc(31), sc(12), sc(32), sc(50), COVER_DARK)

    for y in (22, 28, 34, 40):
        fill(px, sc(15), sc(y), sc(28), sc(y), RULE)
        fill(px, sc(35), sc(y), sc(48), sc(y), RULE)

    fill(px, sc(19), sc(18), sc(23), sc(18), INK)
    fill(px, sc(18), sc(19), sc(18), sc(26), INK)
    fill(px, sc(24), sc(19), sc(24), sc(26), INK)
    fill(px, sc(18), sc(22), sc(24), sc(22), INK)

    fill(px, sc(37), sc(18), sc(47), sc(18), INK)
    fill(px, sc(37), sc(22), sc(47), sc(22), INK)
    fill(px, sc(37), sc(26), sc(47), sc(26), INK)
    fill(px, sc(42), sc(18), sc(42), sc(26), INK)

    fill(px, sc(44), sc(46), sc(47), sc(58), RIBBON)
    return px


def png_bytes(px: list[list[tuple[int, int, int, int]]]) -> bytes:
    h = len(px)
    w = len(px[0])
    raw = b"".join(b"\x00" + bytes(c for x in row for c in x) for row in px)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    return b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)),
            chunk(b"IDAT", zlib.compress(raw, 9)),
            chunk(b"IEND", b""),
        ]
    )


def write_png(path: Path, px: list[list[tuple[int, int, int, int]]]) -> None:
    path.write_bytes(png_bytes(px))


def ico_dib(px: list[list[tuple[int, int, int, int]]]) -> bytes:
    """32-bit BMP DIB with AND mask, as required inside an ICO."""
    h = len(px)
    w = len(px[0])
    xor = bytearray()
    for y in range(h - 1, -1, -1):
        for x in range(w):
            r, g, b, a = px[y][x]
            xor.extend((b, g, r, a))
    row_bytes = ((w + 31) // 32) * 4
    and_mask = bytearray()
    for y in range(h - 1, -1, -1):
        row = bytearray(row_bytes)
        for x in range(w):
            if px[y][x][3] == 0:
                row[x // 8] |= 1 << (7 - (x % 8))
        and_mask.extend(row)
    header = struct.pack(
        "<IIIHHIIIIII",
        40,
        w,
        h * 2,
        1,
        32,
        0,
        len(xor) + len(and_mask),
        0,
        0,
        0,
        0,
    )
    return header + bytes(xor) + bytes(and_mask)


def write_ico(path: Path, sizes: tuple[int, ...]) -> None:
    images = [ico_dib(draw(size)) for size in sizes]
    count = len(images)
    offset = 6 + 16 * count
    entries = bytearray()
    for size, blob in zip(sizes, images):
        w = 0 if size >= 256 else size
        h = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", w, h, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
    path.write_bytes(struct.pack("<HHH", 0, 1, count) + bytes(entries) + b"".join(images))


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, size in (
        ("32x32.png", 32),
        ("128x128.png", 128),
        ("128x128@2x.png", 256),
        ("256x256.png", 256),
        ("512x512.png", 512),
    ):
        write_png(OUT / name, draw(size))
        print(f"wrote {OUT / name}")
    ico = OUT / "icon.ico"
    write_ico(ico, (16, 32, 48, 256))
    print(f"wrote {ico}")


if __name__ == "__main__":
    main()
