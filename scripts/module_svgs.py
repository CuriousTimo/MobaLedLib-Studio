"""Draws the SVGs of the MobaLedLib module library (data/modules/*.svg).

Simple flat top views in one visual style: every module is built from the same few parts
(board, connector, chip, pad, RGB LED, label) and carries a category dot. Proportions and
connector positions follow the boards on wiki.mobaledlib.de, not to the millimetre. Every
connection point is a <g class="port"> with a data-port id (the ids in data/modules/modules.json),
so a chain view can attach wires to it.

lamp.svg and lamp-rgb.svg are written by hand (they are lit through CSS variables).

Usage: python3 scripts/module_svgs.py [out_dir]   (default: data/modules)
"""
import os, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "data/modules"
os.makedirs(OUT, exist_ok=True)

BOARD = "#1F4C8A"
BOARD_EDGE = "#3667AC"
PART = "#14171C"
PIN = "#D9B44A"
TEXT = "#EEF2F7"
LED_PKG = "#E9ECEF"
CUT = "#FF7A6B"
CAT = {"led": "#F5C76A", "splitter": "#5BD48A", "controller": "#5BD48A"}
FONT = 'font-family="JetBrains Mono, ui-monospace, Menlo, monospace"'
LABEL = 5      # every label
TITLE = 6.5    # every module name


def svg(w, h, body, title):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w*2}" height="{h*2}">\n'
            f'<title>{title}</title>\n{body}\n</svg>\n')


def board(w, h):
    return f'<rect x="0.5" y="0.5" width="{w-1}" height="{h-1}" rx="5" fill="{BOARD}" stroke="{BOARD_EDGE}"/>'


def label(x, y, s, anchor="middle", rot=None, fill=TEXT):
    t = f' transform="rotate({rot} {x} {y})"' if rot is not None else ""
    return f'<text x="{x}" y="{y}" font-size="{LABEL}" text-anchor="{anchor}" fill="{fill}" {FONT}{t}>{s}</text>'


def title(x, y, s):
    return f'<text x="{x}" y="{y}" font-size="{TITLE}" text-anchor="middle" fill="{TEXT}" font-weight="700" {FONT}>{s}</text>'


def dot(x, y, cat):
    return f'<circle cx="{x}" cy="{y}" r="3" fill="{CAT[cat]}"/>'


def conn(x, y, port, vertical=False, pins=6):
    """Pin header, 2 rows. Horizontal: wide; vertical: tall."""
    n = pins // 2
    w, h = (n * 5 + 3, 13) if not vertical else (13, n * 5 + 3)
    g = [f'<g class="port" data-port="{port}">',
         f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="1.5" fill="{PART}"/>']
    for i in range(n):
        for j in range(2):
            px, py = (x + 4 + i * 5, y + 4 + j * 5) if not vertical else (x + 4 + j * 5, y + 4 + i * 5)
            g.append(f'<rect x="{px-1}" y="{py-1}" width="2" height="2" fill="{PIN}"/>')
    g.append('</g>')
    return "\n".join(g)


def chip(x, y):
    return f'<rect x="{x}" y="{y}" width="10" height="7" rx="1" fill="{PART}"/>'


def pad(x, y, port=None, w=3, h=6):
    r = f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="1" fill="{PIN}"/>'
    return f'<g class="port" data-port="{port}">{r}</g>' if port else r


def rgb(x, y, s=9, port=None):
    """WS2812 seen from the top."""
    r = (f'<rect x="{x}" y="{y}" width="{s}" height="{s}" rx="1" fill="{LED_PKG}"/>'
         f'<circle cx="{x+s/2}" cy="{y+s/2}" r="{s*0.32}" fill="#FFFFFF" stroke="#AEB4BC" stroke-width="0.5"/>')
    return f'<g class="port" data-port="{port}">{r}</g>' if port else r


def terminals(x, y, n, port_prefix=None):
    g = [f'<rect x="{x}" y="{y}" width="{n*5+2}" height="9" rx="1.5" fill="{PART}"/>']
    for i in range(n):
        g.append(f'<circle cx="{x+3.5+i*5}" cy="{y+4.5}" r="1.5" fill="{PIN}"/>')
    return "\n".join(g)


def cut(x, h):
    return f'<line x1="{x}" y1="0" x2="{x}" y2="{h}" stroke="{CUT}" stroke-width="0.8" stroke-dasharray="3 2"/>'


files = {}

# MLL101 LichtMaschine Classic: LED outputs LED0 and LED2 on the right edge
W, H = 160, 150
b = [board(W, H)]
for i, nx in enumerate((34, 76)):
    b.append(f'<rect x="{nx}" y="8" width="34" height="76" rx="2" fill="{PART}"/>')
    b.append(label(nx + 17, 50, "Nano"))
    b.append(label(nx + 17, 58, "LED" if i == 0 else "DCC"))
b.append(conn(5, 22, "key80", vertical=True, pins=8) + label(5, 17, "KEY80", anchor="start"))
b.append(conn(5, 100, "keyboard", vertical=True, pins=8) + label(5, 95, "KEYBRD", anchor="start"))
b.append(conn(W - 18, 28, "can", vertical=True) + label(W - 24, 37, "CAN", rot=-90))
b.append(conn(W - 18, 68, "led2", vertical=True) + label(W - 24, 77, "LED2", rot=-90, fill=CAT["led"]))
b.append(conn(W - 18, 108, "led0", vertical=True) + label(W - 24, 117, "LED0", rot=-90, fill=CAT["led"]))
b.append(rgb(118, 72, port="hb_led2") + rgb(118, 112, port="hb_led0"))
b.append(title(80, 104, "LichtMaschine (101)"))
for i in range(3):
    b.append(f'<circle cx="{58+i*22}" cy="128" r="6" fill="{PART}"/>')
b.append(dot(W - 8, 8, "controller"))
files["mll101"] = svg(W, H, "\n".join(b), "MLL101 LichtMaschine Classic")

# MLL201 Universal Verteiler Pro: Out 1..12, IN, NEXT, DIP switch, FWD/BACK amplifiers
W, H = 300, 112
b = [board(W, H)]
for i in range(6):
    x = 38 + i * 42
    b.append(conn(x, 4, f"out{2*(i+1)}") + label(x + 9, 24, f"Out {2*(i+1)}"))
    b.append(conn(x, H - 17, f"out{2*i+1}") + label(x + 9, H - 21, f"Out {2*i+1}"))
b.append(conn(4, 42, "in", vertical=True) + label(24, 51, "IN", rot=-90))
b.append(conn(W - 17, 42, "next", vertical=True) + label(W - 23, 51, "NEXT", rot=-90))
b.append(rgb(40, 42, port="led_back") + label(45, 58, "BACK"))
b.append(rgb(40, 62, port="led_fwd") + label(45, 78, "FWD"))
b.append(f'<rect x="78" y="42" width="140" height="26" rx="1.5" fill="{PART}"/>')
for i in range(12):
    sx = 82 + i * 11.2
    b.append(f'<g class="dip" data-dip="{i+1}"><rect x="{sx}" y="45" width="6" height="20" rx="0.8" fill="#3A3F47"/>'
             f'<rect x="{sx+0.8}" y="46" width="4.4" height="8" rx="0.6" fill="{TEXT}"/></g>')
    b.append(label(sx + 3, 76, str(i + 1)))
b.append(label(222, 49, "inactive", anchor="start") + label(222, 66, "active", anchor="start"))
b.append(title(148, 36, "Verteiler Pro (201)"))
b.append(dot(W - 8, 8, "splitter"))
files["mll201"] = svg(W, H, "\n".join(b), "MLL201 Universal Verteiler Pro")

# MLL503 WS2811 Multi-Use v2: connector + 3 cuttable sections of 4 chips (12 x 3 outputs)
SEC = 90
W, H = 26 + 3 * SEC, 76
b = [board(W, H)]
b.append(conn(5, 20, "in", vertical=True))
for s in range(3):
    x0 = 26 + s * SEC
    b.append(cut(x0, H))
    for c in range(4):
        b.append(chip(x0 + 8 + c * 20, 29))
    for p in range(12):
        b.append(pad(x0 + 6 + p * 6.8, 4) + pad(x0 + 6 + p * 6.8, H - 10))
    b.append(label(x0 + SEC / 2, 23, f"Section {s+1}"))
    # solder jumper T1/T2/T3: loops the data back to the input when nothing follows
    b.append(f'<g class="port" data-port="t{s+1}"><circle cx="{x0+SEC-6}" cy="32.5" r="2" fill="{PIN}"/></g>')
    b.append(label(x0 + SEC - 6, 46, f"T{s+1}"))
b.append(f'<g class="port" data-port="dout"><circle cx="{W-6}" cy="55" r="2" fill="{PIN}"/></g>')
b.append(label(W - 10, 57, "Dout", anchor="end"))
b.append(title(26 + SEC * 1.5, 54, "Multi-Use (503)"))
b.append(dot(9, 8, "led"))
files["mll503"] = svg(W, H, "\n".join(b), "MLL503 WS2811 Multi-Use v2")

# MLL504 WS2811 Multi-Use Micro: 2 chips, 6 single LEDs, WS2812B-like pads
W, H = 90, 50
b = [board(W, H)]
for i, (lab, port) in enumerate([("5V", "5v_in"), ("DI", "in"), ("GND", "gnd_in")]):
    b.append(pad(4, 8 + i * 11, port, 5, 5) + label(12, 12 + i * 11, lab, anchor="start"))
for i, (lab, port) in enumerate([("5V", "5v_out"), ("DO", "out"), ("GND", "gnd_out")]):
    b.append(pad(W - 9, 8 + i * 11, port, 5, 5) + label(W - 12, 12 + i * 11, lab, anchor="end"))
b.append(chip(34, 9) + chip(47, 9))
for i in range(6):
    b.append(pad(29 + i * 6, 25))
b.append(title(W / 2, 45, "Micro (504)"))
b.append(dot(W - 7, 43, "led"))
files["mll504"] = svg(W, H, "\n".join(b), "MLL504 WS2811 Multi-Use Micro")

# MLL521 24-Single LED Connector v2: 8 chips, 24 outputs on terminals, IN and OUT/Term
W, H = 240, 96
b = [board(W, H)]
b.append(conn(4, 34, "in", vertical=True) + label(24, 43, "IN", rot=-90))
b.append(conn(W - 17, 34, "out", vertical=True) + label(W - 23, 43, "OUT", rot=-90))
b.append(terminals(40, 6, 16) + label(81, 24, "LED 13-24"))
b.append(terminals(40, H - 15, 16) + label(81, H - 19, "LED 1-12"))
for c in range(8):
    b.append(chip(40 + c * 20, 40))
b.append(title(120, 62, "24 Single LEDs (521)"))
b.append(dot(W - 8, 8, "led"))
files["mll521"] = svg(W, H, "\n".join(b), "MLL521 24-Single LED Connector v2")

# MLL295 TinyHouse: LED1/LED2 single-LED outputs, LED3..LED5 RGB outputs, heartbeat
W, H = 160, 96
b = [board(W, H)]
b.append(conn(4, 34, "in", vertical=True) + label(24, 43, "IN", rot=-90))
for i, n in enumerate([5, 4, 3]):
    x0 = 40 + i * 38
    b.append(f'<g class="port" data-port="rgb{n}">' + "".join(pad(x0 + p * 6, 4) for p in range(4)) + '</g>')
    b.append(label(x0 + 10.5, 19, f"LED{n}"))
b.append(rgb(40, 38, port="heartbeat"))
b.append(chip(80, 39) + chip(110, 39))
for i, n in enumerate([1, 2]):
    x0 = 46 + i * 56
    b.append(f'<g class="port" data-port="led{n}">' + "".join(pad(x0 + p * 7, H - 10) for p in range(6)) + '</g>')
    b.append(label(x0 + 19, H - 14, f"LED{n}"))
b.append(title(100, 62, "TinyHouse (295)"))
b.append(dot(W - 8, 8, "led"))
files["mll295"] = svg(W, H, "\n".join(b), "MLL295 TinyHouse")

# WS2811 breakout (WorldSemi WS2811B, breakaway panel of 10x10). Front side:
# left 4 round LED pads B, G, +, R; right 3 oval pads +, DO, -. DI not on the front (back side?).
W, H = 80, 44
b = [board(W, H)]
for i, lab in enumerate(["B", "G", "+", "R"]):
    port = "led_plus" if lab == "+" else f"led_{lab.lower()}"
    b.append(f'<g class="port" data-port="{port}"><circle cx="8" cy="{7 + i * 9.5}" r="3.2" fill="{PIN}"/></g>')
    b.append(label(15, 9 + i * 9.5, lab, anchor="start"))
b.append(chip(36, 14))
for i, (lab, port) in enumerate([("+", "5v"), ("DO", "out"), ("-", "gnd")]):
    b.append(pad(W - 15, 6 + i * 10, port, 11, 6) + label(W - 18, 11 + i * 10, lab, anchor="end"))
b.append(title(46, 40, "WS2811"))
b.append(dot(W - 7, 39, "led"))
files["ws2811"] = svg(W, H, "\n".join(b), "WS2811 breakout board")

# WS2812: one RGB LED, 3 channels
W, H = 40, 40
b = [f'<rect x="0.5" y="0.5" width="39" height="39" rx="5" fill="{LED_PKG}" stroke="#AEB4BC"/>',
     f'<circle cx="20" cy="18" r="11" fill="#FFFFFF" stroke="#AEB4BC"/>',
     f'<circle cx="16" cy="15" r="2" fill="#E5484D"/><circle cx="24" cy="15" r="2" fill="#30A46C"/><circle cx="20" cy="22" r="2" fill="#3E63DD"/>',
     pad(-1, 15, "in", 3, 6), pad(38, 15, "out", 3, 6),
     f'<text x="20" y="36" font-size="{LABEL}" text-anchor="middle" fill="{PART}" font-weight="700" {FONT}>WS2812</text>',
     dot(34, 6, "led")]
files["ws2812"] = svg(W, H, "\n".join(b), "WS2812 RGB LED")

# Single LED on one WS2811 output, seen from the top (5 mm LED)
W, H = 40, 40
b = [f'<circle cx="20" cy="18" r="13" fill="#FFF3CF" stroke="#E8C66A"/>',
     f'<path d="M 9 11 A 13 13 0 0 0 9 25" fill="none" stroke="#E8C66A" stroke-width="2"/>',
     f'<circle cx="20" cy="18" r="5" fill="#FFE59A"/>',
     f'<text x="20" y="38" font-size="{LABEL}" text-anchor="middle" fill="{TEXT}" font-weight="700" {FONT}>LED</text>',
     f'<g class="port" data-port="in"><circle cx="20" cy="18" r="13" fill="none"/></g>',
     dot(34, 6, "led")]
files["led"] = svg(W, H, "\n".join(b), "Single LED on one WS2811 output")

for k, v in files.items():
    open(os.path.join(OUT, f"{k}.svg"), "w").write(v)
print("written", ", ".join(files))
