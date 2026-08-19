#!/usr/bin/env python3
"""Write the bilingual dictionary mark as PNG icons for cargo-packager."""

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


def write_png(path: Path, px: list[list[tuple[int, int, int, int]]]) -> None:
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

    png = b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)),
            chunk(b"IDAT", zlib.compress(raw, 9)),
            chunk(b"IEND", b""),
        ]
    )
    path.write_bytes(png)


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


if __name__ == "__main__":
    main()
