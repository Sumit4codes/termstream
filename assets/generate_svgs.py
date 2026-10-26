import re, html

ansi_re = re.compile(r"\x1b\[([0-9;]*)m")

def parse_ansi_text(raw_text):
    lines = raw_text.split("\n")
    parsed_lines = []
    
    fg = None
    bg = None
    bold = False
    
    for raw_line in lines:
        raw_line = raw_line.rstrip("\r")
        line_segs = []
        last_idx = 0
        
        for m in ansi_re.finditer(raw_line):
            start, end = m.span()
            if start > last_idx:
                chunk = raw_line[last_idx:start]
                if chunk:
                    line_segs.append((fg, bg, bold, chunk))
            codes = [int(c) for c in m.group(1).split(";") if c]
            if not codes:
                codes = [0]
            i = 0
            while i < len(codes):
                c = codes[i]
                if c == 0:
                    fg = None
                    bg = None
                    bold = False
                elif c == 1:
                    bold = True
                elif c == 38 and i + 4 < len(codes) and codes[i+1] == 2:
                    fg = f"rgb({codes[i+2]},{codes[i+3]},{codes[i+4]})"
                    i += 4
                elif c == 48 and i + 4 < len(codes) and codes[i+1] == 2:
                    bg = f"rgb({codes[i+2]},{codes[i+3]},{codes[i+4]})"
                    i += 4
                elif c == 39:
                    fg = None
                elif c == 49:
                    bg = None
                i += 1
            last_idx = end
            
        if last_idx < len(raw_line):
            chunk = raw_line[last_idx:]
            if chunk:
                line_segs.append((fg, bg, bold, chunk))
        parsed_lines.append(line_segs)
    return parsed_lines

def render_terminal_svg(parsed_lines, title="termstream", font_size=13, char_w=8.2, line_h=17.5, pad_x=22, pad_top=52, pad_bottom=18):
    max_cols = 0
    for line in parsed_lines:
        col = sum(len(txt) for _, _, _, txt in line)
        if col > max_cols:
            max_cols = col
            
    num_rows = len(parsed_lines)
    
    width = int(max_cols * char_w + pad_x * 2)
    height = int(num_rows * line_h + pad_top + pad_bottom)
    
    bg_rects = []
    text_spans = []
    
    for row_idx, line in enumerate(parsed_lines):
        y_pos = pad_top + row_idx * line_h
        cur_col = 0
        
        row_tspans = []
        for fg, bg, bold, text in line:
            text_len = len(text)
            seg_x = pad_x + cur_col * char_w
            seg_w = text_len * char_w
            
            if bg and bg != "rgb(0,0,0)":
                bg_rects.append(f'<rect x="{seg_x:.1f}" y="{y_pos:.1f}" width="{seg_w:.1f}" height="{line_h:.1f}" fill="{bg}"/>')
                
            fill_attr = f' fill="{fg}"' if fg else ' fill="#c9d1d9"'
            weight_attr = ' font-weight="bold"' if bold else ''
            safe_text = html.escape(text)
            
            row_tspans.append(f'<tspan x="{seg_x:.1f}"{fill_attr}{weight_attr}>{safe_text}</tspan>')
            cur_col += text_len
            
        if row_tspans:
            baseline_y = y_pos + line_h * 0.80
            text_spans.append(f'<text y="{baseline_y:.1f}">{"".join(row_tspans)}</text>')
            
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="100%" height="auto">
  <defs>
    <filter id="shadow" x="-5%" y="-5%" width="110%" height="110%">
      <feDropShadow dx="0" dy="8" stdDeviation="16" flood-color="#000000" flood-opacity="0.45"/>
    </filter>
    <linearGradient id="headerGrad" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#1f242c"/>
      <stop offset="100%" stop-color="#161b22"/>
    </linearGradient>
  </defs>

  <style>
    .window-bg {{ fill: #0d1117; stroke: #30363d; stroke-width: 1px; rx: 10px; }}
    .title-text {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 12px; font-weight: 500; fill: #8b949e; text-anchor: middle; }}
    text {{ font-family: "JetBrains Mono", "Fira Code", "SF Mono", Menlo, Monaco, Consolas, "Liberation Mono", monospace; font-size: {font_size}px; font-feature-settings: "liga" 0; }}
  </style>

  <!-- Window container -->
  <g filter="url(#shadow)">
    <rect x="1" y="1" width="{width - 2}" height="{height - 2}" class="window-bg"/>
    
    <!-- Titlebar -->
    <path d="M 1,11 A 10,10 0 0,1 11,1 L {width - 11},1 A 10,10 0 0,1 {width - 1},11 L {width - 1},36 L 1,36 Z" fill="url(#headerGrad)"/>
    <line x1="1" y1="36" x2="{width - 1}" y2="36" stroke="#21262d" stroke-width="1"/>
    
    <!-- Window controls -->
    <circle cx="20" cy="18" r="5.5" fill="#ff5f56" stroke="#e0443e" stroke-width="0.5"/>
    <circle cx="36" cy="18" r="5.5" fill="#ffbd2e" stroke="#dea123" stroke-width="0.5"/>
    <circle cx="52" cy="18" r="5.5" fill="#27c93f" stroke="#1aab29" stroke-width="0.5"/>
    
    <!-- Window title -->
    <text x="{width / 2}" y="22" class="title-text">{html.escape(title)}</text>
  </g>

  <!-- Cell backgrounds -->
  <g>
    {''.join(bg_rects)}
  </g>

  <!-- Monospace text layer -->
  <g xml:space="preserve">
    {''.join(text_spans)}
  </g>
</svg>'''
    return svg

def generate_all():
    # 1. Hero Player SVG
    with open("/tmp/opencode_hero_frame.txt") as f:
        hero_text = f.read()
    hero_parsed = parse_ansi_text(hero_text)
    hero_svg = render_terminal_svg(hero_parsed, title="termstream — 82×24 — Quadrant 2x2 Hi-Res TrueColor", font_size=13, char_w=8.4, line_h=18)
    with open("assets/hero_player.svg", "w") as f:
        f.write(hero_svg)

    # 2. Storyboard SVG
    with open("/tmp/opencode_storyboard.txt") as f:
        storyboard_text = f.read()
    storyboard_parsed = parse_ansi_text(storyboard_text)
    storyboard_svg = render_terminal_svg(storyboard_parsed, title="termstream --grid test:bars — 3×3 Video Storyboard Contact Sheet", font_size=12, char_w=7.8, line_h=16.5)
    with open("assets/storyboard_grid.svg", "w") as f:
        f.write(storyboard_svg)

    # 3. Modes Showcase SVG
    with open("/tmp/opencode_mode_quadrant.txt") as f:
        quad_p = parse_ansi_text(f.read())
    with open("/tmp/opencode_mode_braille.txt") as f:
        braille_p = parse_ansi_text(f.read())
    with open("/tmp/opencode_mode_matrix.txt") as f:
        matrix_p = parse_ansi_text(f.read())
    with open("/tmp/opencode_mode_halfblock.txt") as f:
        half_p = parse_ansi_text(f.read())

    def render_card_contents(parsed_lines, start_x, start_y, char_w=7.2, line_h=15.5):
        bg_rects = []
        text_spans = []
        for r_idx, line in enumerate(parsed_lines):
            y_pos = start_y + r_idx * line_h
            cur_col = 0
            for fg, bg, bold, text in line:
                text_len = len(text)
                seg_x = start_x + cur_col * char_w
                seg_w = text_len * char_w
                if bg and bg != "rgb(0,0,0)":
                    bg_rects.append(f'<rect x="{seg_x:.1f}" y="{y_pos:.1f}" width="{seg_w:.1f}" height="{line_h:.1f}" fill="{bg}"/>')
                fill_attr = f' fill="{fg}"' if fg else ' fill="#c9d1d9"'
                weight_attr = ' font-weight="bold"' if bold else ''
                safe_text = html.escape(text)
                text_spans.append(f'<tspan x="{seg_x:.1f}"{fill_attr}{weight_attr}>{safe_text}</tspan>')
                cur_col += text_len
            if line:
                baseline = y_pos + line_h * 0.8
                text_spans.append(f'</text><text y="{baseline:.1f}">')
        return "".join(bg_rects), f'<text y="{start_y + line_h * 0.8:.1f}">' + "".join(text_spans) + '</text>'

    card_w = 420
    card_h = 240
    gap = 20
    total_w = card_w * 2 + gap + 30
    total_h = card_h * 2 + gap + 30

    cards_info = [
        (15, 15, "Quadrant 2x2 Hi-Res (4 Subpixels/Cell)", quad_p, "#58a6ff"),
        (15 + card_w + gap, 15, "Adaptive Braille 2x4 (8 Dots/Cell)", braille_p, "#bc8cff"),
        (15, 15 + card_h + gap, "Retro Phosphor Matrix CRT Filter", matrix_p, "#3fb950"),
        (15 + card_w + gap, 15 + card_h + gap, "Half-Block (TrueColor ▀)", half_p, "#f0883e"),
    ]

    cards_svg = []
    for cx, cy, title, p_lines, badge_col in cards_info:
        bgs, txt = render_card_contents(p_lines, cx + 16, cy + 42)
        card_svg = f"""
        <g>
          <rect x="{cx}" y="{cy}" width="{card_w}" height="{card_h}" rx="8" fill="#0d1117" stroke="#30363d" stroke-width="1"/>
          <rect x="{cx}" y="{cy}" width="{card_w}" height="32" rx="8" fill="#161b22"/>
          <path d="M {cx},{cy+24} L {cx},{cy+32} L {cx+card_w},{cy+32} L {cx+card_w},{cy+24} Z" fill="#161b22"/>
          <line x1="{cx}" y1="{cy+32}" x2="{cx+card_w}" y2="{cy+32}" stroke="#21262d" stroke-width="1"/>
          <circle cx="{cx+16}" cy="{cy+16}" r="4" fill="{badge_col}"/>
          <text x="{cx+28}" y="{cy+20}" font-family="-apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="12" font-weight="600" fill="#e6edf3">{title}</text>
          {bgs}
          <g xml:space="preserve">{txt}</g>
        </g>
        """
        cards_svg.append(card_svg)

    full_svg = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total_w} {total_h}" width="100%" height="auto">
      <style>
        text {{ font-family: "JetBrains Mono", "Fira Code", monospace; font-size: 11px; }}
      </style>
      <rect width="{total_w}" height="{total_h}" fill="#010409" rx="12"/>
      {"".join(cards_svg)}
    </svg>"""

    with open("assets/modes_showcase.svg", "w") as f:
        f.write(full_svg)

    print("All assets generated successfully in assets/!")

if __name__ == "__main__":
    generate_all()
