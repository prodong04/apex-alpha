import json

with open('/Users/elias/apex-alpha/reports/real_equity_curve.json') as f:
    data = json.load(f)

daily_eq = data['daily_equity']
dates = list(daily_eq.keys())
equities = list(daily_eq.values())

peak = 10000.0
drawdowns = []
for eq in equities:
    if eq > peak:
        peak = eq
    dd = ((eq - peak) / peak) * 100.0
    drawdowns.append(round(dd, 2))

# SVG Dimensions
w = 1200
h = 420
pad_l = 85
pad_r = 75
pad_t = 40
pad_b = 50

plot_w = w - pad_l - pad_r
plot_h = h - pad_t - pad_b

min_eq = 9500.0
max_eq = 18500.0

def scale_x(i):
    return pad_l + (i / (len(equities) - 1)) * plot_w

def scale_y_eq(v):
    return pad_t + (1.0 - (v - min_eq) / (max_eq - min_eq)) * plot_h

def scale_y_dd(v):
    # dd is between -15 and 0
    clamped = max(-15.0, min(0.0, v))
    return pad_t + (1.0 - (clamped - (-15.0)) / (15.0)) * plot_h

# Build path for Equity
pts_eq = [f"{scale_x(i):.1f},{scale_y_eq(v):.1f}" for i, v in enumerate(equities)]
eq_path = "M " + " L ".join(pts_eq)
eq_area = f"{eq_path} L {scale_x(len(equities)-1):.1f},{pad_t+plot_h:.1f} L {scale_x(0):.1f},{pad_t+plot_h:.1f} Z"

# Build path for Drawdown
pts_dd = [f"{scale_x(i):.1f},{scale_y_dd(v):.1f}" for i, v in enumerate(drawdowns)]
dd_path = "M " + " L ".join(pts_dd)
dd_zero_y = scale_y_dd(0.0)
dd_area = f"{dd_path} L {scale_x(len(drawdowns)-1):.1f},{dd_zero_y:.1f} L {scale_x(0):.1f},{dd_zero_y:.1f} Z"

# Grid lines & ticks
grid_lines = []
for v in [10000, 12000, 14000, 16000, 18000]:
    y = scale_y_eq(v)
    grid_lines.append(f'<line x1="{pad_l}" y1="{y:.1f}" x2="{w-pad_r}" y2="{y:.1f}" stroke="rgba(30,45,74,0.6)" stroke-dasharray="4 4"/>')
    grid_lines.append(f'<text x="{pad_l-12}" y="{y+4:.1f}" fill="#00f2fe" font-size="12" font-family="monospace" text-anchor="end">${v:,}</text>')

# Right Y grid (Drawdown)
for dd_val in [0, -5, -10, -15]:
    y = scale_y_dd(dd_val)
    grid_lines.append(f'<text x="{w-pad_r+12}" y="{y+4:.1f}" fill="#ef4444" font-size="12" font-family="monospace" text-anchor="start">{dd_val}%</text>')

# X ticks (Quarterly)
x_ticks = []
step_dates = len(dates) // 6
for idx in range(0, len(dates), step_dates):
    x = scale_x(idx)
    d_str = dates[idx]
    x_ticks.append(f'<line x1="{x:.1f}" y1="{pad_t+plot_h}" x2="{x:.1f}" y2="{pad_t+plot_h+6}" stroke="#64748b"/>')
    x_ticks.append(f'<text x="{x:.1f}" y="{pad_t+plot_h+22}" fill="#94a3b8" font-size="12" font-family="monospace" text-anchor="middle">{d_str}</text>')

grid_svg = "\n".join(grid_lines)
ticks_svg = "\n".join(x_ticks)

svg_markup = f'''
<div style="width:100%; overflow-x:auto; background: #0b111e; border-radius: 12px; padding: 16px; border: 1px solid #1e2d4a; margin-top: 12px;">
<svg viewBox="0 0 {w} {h}" style="width:100%; min-width: 800px; height:auto; display:block;">
    <defs>
        <linearGradient id="eqGrad" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stop-color="#00f2fe" stop-opacity="0.3"/>
            <stop offset="100%" stop-color="#00f2fe" stop-opacity="0.0"/>
        </linearGradient>
        <linearGradient id="ddGrad" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stop-color="#ef4444" stop-opacity="0.35"/>
            <stop offset="100%" stop-color="#ef4444" stop-opacity="0.05"/>
        </linearGradient>
    </defs>
    
    <!-- Background Frame -->
    <rect x="{pad_l}" y="{pad_t}" width="{plot_w}" height="{plot_h}" fill="rgba(10,14,23,0.8)" stroke="#1e2d4a" rx="6"/>
    
    <!-- Grids -->
    {grid_svg}
    {ticks_svg}
    
    <!-- Drawdown Area & Line -->
    <path d="{dd_area}" fill="url(#ddGrad)"/>
    <path d="{dd_path}" fill="none" stroke="#ef4444" stroke-width="1.8" opacity="0.9"/>
    
    <!-- Equity Area & Line -->
    <path d="{eq_area}" fill="url(#eqGrad)"/>
    <path d="{eq_path}" fill="none" stroke="#00f2fe" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/>
    
    <!-- Legend -->
    <rect x="{pad_l+16}" y="{pad_t+12}" width="380" height="28" fill="rgba(19,27,46,0.8)" rx="4" stroke="#1e2d4a"/>
    <circle cx="{pad_l+30}" cy="{pad_t+26}" r="5" fill="#00f2fe"/>
    <text x="{pad_l+42}" y="{pad_t+30}" fill="#f0f4f8" font-size="12" font-weight="700">Real Portfolio Equity ($) [Final: $17,867.91 | +78.7%]</text>
    
    <circle cx="{pad_l+420}" cy="{pad_t+26}" r="5" fill="#ef4444"/>
    <text x="{pad_l+432}" y="{pad_t+30}" fill="#ef4444" font-size="12" font-weight="700">Real Drawdown (%) [MDD: -12.97%]</text>
</svg>
</div>
'''

with open('/Users/elias/apex-alpha/reports/stat_arb_executive_tearsheet.html') as f:
    html = f.read()

# Replace chart-section container
idx_start = html.find('<div class="chart-container">')
idx_end = html.find('</div>', idx_start) + 6

if idx_start != -1:
    new_html = html[:idx_start] + svg_markup + html[idx_end:]
    with open('/Users/elias/apex-alpha/reports/stat_arb_executive_tearsheet.html', 'w') as f:
        f.write(new_html)
    print("✅ Successfully updated HTML with pure vector self-contained SVG chart!")

standalone_svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" style="background:#0a0e17;">
    <defs>
        <linearGradient id="eqGrad2" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stop-color="#00f2fe" stop-opacity="0.35"/>
            <stop offset="100%" stop-color="#00f2fe" stop-opacity="0.0"/>
        </linearGradient>
        <linearGradient id="ddGrad2" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stop-color="#ef4444" stop-opacity="0.35"/>
            <stop offset="100%" stop-color="#ef4444" stop-opacity="0.05"/>
        </linearGradient>
    </defs>
    
    <rect width="100%" height="100%" fill="#0a0e17"/>
    <rect x="{pad_l}" y="{pad_t}" width="{plot_w}" height="{plot_h}" fill="rgba(19,27,46,0.6)" stroke="#1e2d4a" rx="6"/>
    
    {grid_svg}
    {ticks_svg}
    
    <path d="{dd_area}" fill="url(#ddGrad2)"/>
    <path d="{dd_path}" fill="none" stroke="#ef4444" stroke-width="1.8" opacity="0.9"/>
    
    <path d="{eq_area}" fill="url(#eqGrad2)"/>
    <path d="{eq_path}" fill="none" stroke="#00f2fe" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/>
    
    <rect x="{pad_l+16}" y="{pad_t+12}" width="380" height="28" fill="rgba(10,14,23,0.85)" rx="4" stroke="#1e2d4a"/>
    <circle cx="{pad_l+30}" cy="{pad_t+26}" r="5" fill="#00f2fe"/>
    <text x="{pad_l+42}" y="{pad_t+30}" fill="#f0f4f8" font-size="12" font-weight="700" font-family="sans-serif">Portfolio Equity ($) [+$7,867.91 / +78.7%]</text>
    
    <circle cx="{pad_l+420}" cy="{pad_t+26}" r="5" fill="#ef4444"/>
    <text x="{pad_l+432}" y="{pad_t+30}" fill="#ef4444" font-size="12" font-weight="700" font-family="sans-serif">Drawdown (%) [MDD: -12.97%]</text>
</svg>'''

with open('/Users/elias/apex-alpha/reports/stat_arb_equity_curve.svg', 'w') as f:
    f.write(standalone_svg)
print("✅ Generated standalone SVG: /Users/elias/apex-alpha/reports/stat_arb_equity_curve.svg")
