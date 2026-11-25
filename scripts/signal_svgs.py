"""Writes the DB light signal SVGs of the module library (data/modules/signal-*.svg) from Hive-Rail's
signal faces (https://github.com/CuriousTimo/Hive-Rail, assets/signals; same author).

Hive-Rail shows an aspect through CSS classes on the whole drawing. Here every lamp becomes a
<g class="port" data-port="..."> with a lit overlay whose opacity is the CSS variable --<port> (0..1),
so the chain view can light each lamp from the WS2811/WS2812 output that drives it (the "drives" of
the signal in data/modules/modules.json). Optional lamps (Sh 1 on a Ks signal, the Zusatzlicht) carry
data-option and show only when the drawing has the class sh-lamp or extra-lamp.

Usage: python3 scripts/signal_svgs.py <Hive-Rail>/assets/signals [out_dir]   (default: data/modules)
"""
import os
import re
import sys

src = sys.argv[1]
out = sys.argv[2] if len(sys.argv) > 2 else "data/modules"
os.makedirs(out, exist_ok=True)

COLOR = {"red": "#ff2a1f", "green": "#2fe05a", "yellow": "#ffbf1a", "white": "#f6f6f6", "extra": "#f6f6f6"}
SKIP = {"sf-lamp", "sf-option", "extra-lamp", "sh-lamp"}
for f in sorted(os.listdir(src)):
    if not f.endswith(".svg"):
        continue
    s = open(os.path.join(src, f)).read()
    seen = {}
    ports = []
    def lamp(m):
        tag = m.group(0)
        cls = re.search(r'class="([^"]*)"', tag).group(1).split()
        name = "-".join(c[3:] for c in cls if c not in SKIP)
        seen[name] = seen.get(name, 0) + 1
        port = name if seen[name] == 1 else f"{name}{seen[name]}"
        col = next(v for k, v in COLOR.items() if k in name)
        cx, cy, r = (re.search(fr'{a}="([^"]*)"', tag).group(1) for a in ("cx", "cy", "r"))
        opt = next((c for c in cls if c in ("extra-lamp", "sh-lamp")), None)
        ports.append(port)
        lit = (f'<circle class="sf-lit" cx="{cx}" cy="{cy}" r="{r}" fill="{col}" '
               f'style="opacity: var(--{port}, 0)" pointer-events="none"/>')
        tag = tag.replace(" sf-option", "").replace(" extra-lamp", "").replace(" sh-lamp", "")
        dopt = f' data-option="{opt}"' if opt else ""
        return f'<g class="port" data-port="{port}"{dopt}>{tag}{lit}</g>'
    # rename duplicate first so later lamps get suffixes in drawing order
    s2 = re.sub(r'<circle class="sf-lamp[^"]*"[^>]*/>', lamp, s)
    s2 = s2.replace('.sf-option { display: none; }', '.sf-option { display: none; }\n  g[data-option] { display: none; }\n  .sh-lamp g[data-option="sh-lamp"], .extra-lamp g[data-option="extra-lamp"] { display: inline; }')
    s2 = s2.replace('<title>', f'<!-- From Hive-Rail assets/signals/{f} (same author); each lamp lit by: --<port> = 0..1 -->\n  <title>', 1)
    open(os.path.join(out, "signal-" + f), "w").write(s2)
    print(f, ports)
