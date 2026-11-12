#!/usr/bin/env python3
"""Rasterise the WhatsApp glyph into WhaRa's icon set.

The path data is the Simple Icons release of the WhatsApp mark (CC0-1.0). To
avoid a browser or an SVG library, this parses the subset of SVG path syntax
that the glyph actually uses — moveto, lineto, cubic bezier and elliptical arc —
flattens it to polygons, and fills them with a scanline rasteriser.

The published glyph is an outline drawing, so the two contours that matter here
are the outer speech bubble (filled with the brand gradient) and the handset
(punched out to white). That reproduces the familiar app mark.
"""

import math
import os
import struct
import zlib

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src-tauri", "icons")
MASTER = 1024

# Simple Icons: whatsapp.svg (CC0-1.0), 24x24 viewBox.
GLYPH_VIEWBOX = 24.0
GLYPH_PATH = (
    "M17.472 14.382c-.297-.149-1.758-.867-2.03-.967-.273-.099-.471-.148-.67.15"
    "-.197.297-.767.966-.94 1.164-.173.199-.347.223-.644.075-.297-.15-1.255"
    "-.463-2.39-1.475-.883-.788-1.48-1.761-1.653-2.059-.173-.297-.018-.458"
    ".13-.606.134-.133.298-.347.446-.52.149-.174.198-.298.298-.497.099-.198"
    ".05-.371-.025-.52-.075-.149-.669-1.612-.916-2.207-.242-.579-.487-.5-.669"
    "-.51-.173-.008-.371-.01-.57-.01-.198 0-.52.074-.792.372-.272.297-1.04"
    " 1.016-1.04 2.479 0 1.462 1.065 2.875 1.213 3.074.149.198 2.096 3.2"
    " 5.077 4.487.709.306 1.262.489 1.694.625.712.227 1.36.195 1.871.118"
    ".571-.085 1.758-.719 2.006-1.413.248-.694.248-1.289.173-1.413-.074"
    "-.124-.272-.198-.57-.347m-5.421 7.403h-.004a9.87 9.87 0 01-5.031"
    "-1.378l-.361-.214-3.741.982.998-3.648-.235-.374a9.86 9.86 0 01-1.51"
    "-5.26c.001-5.45 4.436-9.884 9.888-9.884 2.64 0 5.122 1.03 6.988"
    " 2.898a9.825 9.825 0 012.893 6.994c-.003 5.45-4.437 9.884-9.885"
    " 9.884m8.413-18.297A11.815 11.815 0 0012.05 0C5.495 0 .16 5.335"
    " .157 11.892c0 2.096.547 4.142 1.588 5.945L.057 24l6.305-1.654a"
    "11.882 11.882 0 005.683 1.448h.005c6.554 0 11.89-5.335 11.893"
    "-11.893a11.821 11.821 0 00-3.48-8.413Z"
)

# Fraction of the canvas the glyph occupies, as padding on each side.
GLYPH_INSET = 0.11

GREEN_TOP = (37, 211, 102)
GREEN_BOTTOM = (17, 165, 76)

COMMAND = set("MmLlHhVvCcSsQqTtAaZz")
ARCS = 64
CURVES = 48


def write_png(path, width, height, pixels):
    raw = bytearray()
    for row in pixels:
        raw.append(0)
        for r, g, b, a in row:
            raw += bytes((r, g, b, a))

    def chunk(tag, data):
        out = struct.pack(">I", len(data)) + tag + data
        return out + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as fh:
        fh.write(png)


# ---- path parsing ---------------------------------------------------------------------------

def tokenize(path):
    """Split path data into commands and numbers.

    A sign always starts a new number and a second dot does too, so "-.67.15"
    is two numbers rather than one malformed one.
    """
    tokens, buf = [], ""

    def flush():
        nonlocal buf
        if buf:
            tokens.append(buf)
            buf = ""

    for ch in path:
        if ch in COMMAND:
            flush()
            tokens.append(ch)
        elif ch in " \t\r\n":
            flush()
        elif ch in "+-":
            flush()
            buf = ch
        elif ch in "0123456789":
            buf += ch
        elif ch == ".":
            if "." in buf or "e" in buf or "E" in buf:
                flush()
            buf += ch
        elif ch in "eE":
            buf += ch
        else:
            raise ValueError(f"unexpected character {ch!r}")

    flush()
    return tokens


def arc_points(cx, cy, rx, ry, rotation, large, sweep, x, y, steps=ARCS):
    """Flatten an elliptical arc, following the SVG implementation notes."""
    if rx == 0 or ry == 0:
        return [(x, y)]

    phi = math.radians(rotation)
    cos_p, sin_p = math.cos(phi), math.sin(phi)

    dx2, dy2 = (cx - x) / 2.0, (cy - y) / 2.0
    x1p = cos_p * dx2 + sin_p * dy2
    y1p = -sin_p * dx2 + cos_p * dy2

    lam = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry)
    if lam > 1:
        root = math.sqrt(lam)
        rx *= root
        ry *= root

    num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p
    den = rx * rx * y1p * y1p + ry * ry * x1p * x1p
    factor = math.sqrt(max(0.0, num / den)) if den else 0.0
    if large == sweep:
        factor = -factor

    cxp = factor * rx * y1p / ry
    cyp = -factor * ry * x1p / rx
    ccx = cos_p * cxp - sin_p * cyp + (cx + x) / 2.0
    ccy = sin_p * cxp + cos_p * cyp + (cy + y) / 2.0

    def angle(ux, uy, vx, vy):
        dot = ux * vx + uy * vy
        length = math.hypot(ux, uy) * math.hypot(vx, vy)
        value = max(-1.0, min(1.0, dot / length)) if length else 1.0
        out = math.acos(value)
        return -out if (ux * vy - uy * vx) < 0 else out

    theta1 = angle(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
    delta = angle(
        (x1p - cxp) / rx, (y1p - cyp) / ry,
        (-x1p - cxp) / rx, (-y1p - cyp) / ry,
    )
    if not sweep and delta > 0:
        delta -= 2 * math.pi
    elif sweep and delta < 0:
        delta += 2 * math.pi

    points = []
    for i in range(1, steps + 1):
        t = theta1 + delta * (i / steps)
        points.append((
            ccx + rx * math.cos(t) * cos_p - ry * math.sin(t) * sin_p,
            ccy + rx * math.cos(t) * sin_p + ry * math.sin(t) * cos_p,
        ))
    return points


def flatten(path):
    """Return every closed sub-path as a list of points."""
    tokens = tokenize(path)
    subs, current = [], []
    pos = start = (0.0, 0.0)
    command = None
    i = 0

    def num():
        nonlocal i
        value = float(tokens[i])
        i += 1
        return value

    def flag():
        """Arc flags are single digits and may be glued to the next value."""
        nonlocal i
        token = tokens[i]
        if len(token) > 1 and token[0] in "01":
            tokens[i] = token[1:]
            return int(token[0])
        i += 1
        return int(token[0])

    while i < len(tokens):
        token = tokens[i]
        if token in COMMAND:
            command = token
            i += 1
            if command in "Zz":
                if current:
                    current.append(start)
                    subs.append(current)
                    current = []
                pos = start
                command = None
                continue
        elif command is None:
            i += 1
            continue

        if command in "Mm":
            x, y = num(), num()
            if command == "m":
                x, y = pos[0] + x, pos[1] + y
            if current:
                subs.append(current)
            pos = start = (x, y)
            current = [pos]
            command = "L" if command == "M" else "l"
        elif command in "Ll":
            x, y = num(), num()
            if command == "l":
                x, y = pos[0] + x, pos[1] + y
            pos = (x, y)
            current.append(pos)
        elif command in "Hh":
            x = num()
            if command == "h":
                x += pos[0]
            pos = (x, pos[1])
            current.append(pos)
        elif command in "Vv":
            y = num()
            if command == "v":
                y += pos[1]
            pos = (pos[0], y)
            current.append(pos)
        elif command in "Cc":
            x1, y1, x2, y2, x, y = (num() for _ in range(6))
            if command == "c":
                x1 += pos[0]; y1 += pos[1]
                x2 += pos[0]; y2 += pos[1]
                x += pos[0]; y += pos[1]
            for s in range(1, CURVES + 1):
                t = s / CURVES
                mt = 1 - t
                current.append((
                    mt**3 * pos[0] + 3 * mt**2 * t * x1 + 3 * mt * t**2 * x2 + t**3 * x,
                    mt**3 * pos[1] + 3 * mt**2 * t * y1 + 3 * mt * t**2 * y2 + t**3 * y,
                ))
            pos = (x, y)
        elif command in "SsQqTt":
            raise ValueError(f"unsupported command {command}")
        elif command in "Aa":
            rx, ry, rotation = num(), num(), num()
            large, sweep = flag(), flag()
            x, y = num(), num()
            if command == "a":
                x, y = pos[0] + x, pos[1] + y
            current.extend(arc_points(
                pos[0], pos[1], abs(rx), abs(ry),
                rotation, large, sweep, x, y,
            ))
            pos = (x, y)
        else:
            raise ValueError(f"unsupported command {command}")

    if current:
        subs.append(current)
    return subs


def shoelace(sub):
    total = 0.0
    for k in range(len(sub)):
        x1, y1 = sub[k]
        x2, y2 = sub[(k + 1) % len(sub)]
        total += x1 * y2 - x2 * y1
    return abs(total) / 2.0


def contours(path):
    """Split the glyph into the outer bubble and the handset.

    The published mark is an outline, so its largest contour is the bubble and
    the next largest is the ring that turns it into a stroke. The handset is the
    small one, and it is what gets punched out to white.
    """
    subs = flatten(path)
    ranked = sorted(subs, key=shoelace, reverse=True)
    bubble = ranked[0]
    handset = ranked[-1]
    return bubble, handset


# ---- rasterising ----------------------------------------------------------------------------

def scale(sub, size):
    span = size * (1.0 - 2 * GLYPH_INSET)
    factor = span / GLYPH_VIEWBOX
    shift = size * GLYPH_INSET
    return [(shift + x * factor, shift + y * factor) for x, y in sub]


def fill(sub, size, samples=4):
    """Even-odd scanline fill, supersampled vertically for smooth edges.

    Returns a flat list of `size * size` coverage values in 0..1.
    """
    edges = []
    n = len(sub)
    for k in range(n):
        x1, y1 = sub[k]
        x2, y2 = sub[(k + 1) % n]
        if y1 == y2:
            continue
        edges.append((y1, x1, y2, x2))

    if not edges:
        return [0.0] * (size * size)

    acc = [0.0] * (size * size)
    step = 1.0 / samples

    for row in range(size):
        base = row * size
        for s in range(samples):
            sy = row + (s + 0.5) * step

            crossings = []
            for (y1, x1, y2, x2) in edges:
                if (y1 <= sy < y2) or (y2 <= sy < y1):
                    t = (sy - y1) / (y2 - y1)
                    crossings.append(x1 + t * (x2 - x1))

            if not crossings:
                continue

            crossings.sort()
            weight = step
            for k in range(0, len(crossings) - 1, 2):
                lo = crossings[k]
                hi = crossings[k + 1]
                if hi <= 0 or lo >= size:
                    continue
                start = max(0, int(math.ceil(lo - 0.5)))
                end = min(size - 1, int(math.floor(hi - 0.5)))
                for x in range(start, end + 1):
                    acc[base + x] += weight

    return acc


def render(size, bubble, handset):
    outer = fill(scale(bubble, size), size)
    inner = fill(scale(handset, size), size)

    rows = []
    for y in range(size):
        row = []
        t = y / max(1, size - 1)
        r = GREEN_TOP[0] * (1 - t) + GREEN_BOTTOM[0] * t
        g = GREEN_TOP[1] * (1 - t) + GREEN_BOTTOM[1] * t
        b = GREEN_TOP[2] * (1 - t) + GREEN_BOTTOM[2] * t

        for x in range(size):
            k = y * size + x
            body = outer[k]
            if body <= 0:
                row.append((0, 0, 0, 0))
                continue

            # The handset is punched out to white.
            cut = min(1.0, inner[k])
            row.append((
                int(r * (1 - cut) + 255 * cut),
                int(g * (1 - cut) + 255 * cut),
                int(b * (1 - cut) + 255 * cut),
                int(255 * min(1.0, body)),
            ))
        rows.append(row)
    return rows


def render_mask(size, bubble, handset):
    outer = fill(scale(bubble, size), size)
    inner = fill(scale(handset, size), size)

    rows = []
    for y in range(size):
        row = []
        for x in range(size):
            k = y * size + x
            a = max(0.0, min(1.0, outer[k]) - min(1.0, inner[k]))
            row.append((0, 0, 0, int(255 * a)))
        rows.append(row)
    return rows


def main():
    os.makedirs(OUT, exist_ok=True)

    bubble, handset = contours(GLYPH_PATH)

    write_png(os.path.join(OUT, "icon.png"), MASTER, MASTER, render(MASTER, bubble, handset))

    for edge in (32, 128, 256, 512):
        write_png(
            os.path.join(OUT, f"{edge}x{edge}.png"),
            edge, edge, render(edge, bubble, handset),
        )

    write_png(os.path.join(OUT, "128x128@2x.png"), 256, 256, render(256, bubble, handset))
    write_png(os.path.join(OUT, "tray.png"), 44, 44, render_mask(44, bubble, handset))
    write_png(os.path.join(OUT, "tray@2x.png"), 88, 88, render_mask(88, bubble, handset))

    print("icons written to", os.path.normpath(OUT))


if __name__ == "__main__":
    main()