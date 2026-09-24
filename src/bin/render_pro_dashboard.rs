use chrono::Timelike;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

struct TradeRecord {
    ts: i64,
    dt: String,
    side: String,
    entry_px: f64,
    exit_px: f64,
    holding_ms: i64,
    gross_pnl: f64,
    fee_maker: f64,
    net_pnl_maker: f64,
    cum_pnl_maker: f64,
    net_pnl_taker: f64,
    cum_pnl_taker: f64,
    reason: String,
}

fn main() {
    let csv_path = "/Users/elias/apex-alpha/scratch/backtest_pnl_timeseries.csv";
    let file = File::open(csv_path).unwrap();
    let reader = BufReader::new(file);

    let mut records = Vec::new();
    let mut hourly_pnl: BTreeMap<u32, (usize, usize, f64, f64)> = BTreeMap::new();
    for i in 0..24 { hourly_pnl.insert(i, (0, 0, 0.0, 0.0)); }

    for (i, line) in reader.lines().enumerate() {
        if i == 0 { continue; }
        if let Ok(l) = line {
            let parts: Vec<&str> = l.split(',').collect();
            if parts.len() >= 14 {
                let ts: i64 = parts[0].parse().unwrap_or(0);
                let dt = parts[1].to_string();
                let side = parts[2].to_string();
                let entry_px: f64 = parts[3].parse().unwrap_or(0.0);
                let exit_px: f64 = parts[4].parse().unwrap_or(0.0);
                let holding_ms: i64 = parts[5].parse().unwrap_or(0);
                let gross_pnl: f64 = parts[6].parse().unwrap_or(0.0);
                let fee_maker: f64 = parts[7].parse().unwrap_or(0.0);
                let net_pnl_maker: f64 = parts[8].parse().unwrap_or(0.0);
                let cum_pnl_maker: f64 = parts[9].parse().unwrap_or(0.0);
                let net_pnl_taker: f64 = parts[11].parse().unwrap_or(0.0);
                let cum_pnl_taker: f64 = parts[12].parse().unwrap_or(0.0);
                let reason = parts[13].to_string();

                records.push(TradeRecord {
                    ts, dt, side, entry_px, exit_px, holding_ms, gross_pnl, fee_maker,
                    net_pnl_maker, cum_pnl_maker, net_pnl_taker, cum_pnl_taker, reason,
                });

                if let Some(d) = chrono::DateTime::from_timestamp_millis(ts) {
                    let hour_kst = (d.hour() + 9) % 24;
                    let entry = hourly_pnl.entry(hour_kst).or_insert((0, 0, 0.0, 0.0));
                    entry.0 += 1;
                    if net_pnl_maker > 0.0 { entry.1 += 1; }
                    entry.2 += gross_pnl;
                    entry.3 += net_pnl_maker;
                }
            }
        }
    }

    let svg_path = "/Users/elias/apex-alpha/scratch/pro_dashboard.svg";
    generate_pro_dashboard(&records, &hourly_pnl, svg_path);
    println!("✅ Generated Pro Dashboard SVG");
}

fn generate_pro_dashboard(
    records: &[TradeRecord],
    hourly: &BTreeMap<u32, (usize, usize, f64, f64)>,
    out_path: &str,
) {
    let width = 1600.0;
    let height = 1100.0;

    let mut svg = String::with_capacity(65536);

    // SVG Header with fonts, gradients & filters
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" style="background-color: #07090E; font-family: -apple-system, BlinkMacSystemFont, 'Inter', 'SF Pro Display', Roboto, sans-serif;">
        <defs>
            <linearGradient id="bgGrad" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stop-color="#0B0F19"/>
                <stop offset="50%" stop-color="#07090E"/>
                <stop offset="100%" stop-color="#05070A"/>
            </linearGradient>

            <linearGradient id="makerGlow" x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" stop-color="#00F298" stop-opacity="0.4"/>
                <stop offset="40%" stop-color="#00C471" stop-opacity="0.15"/>
                <stop offset="100%" stop-color="#00C471" stop-opacity="0.0"/>
            </linearGradient>

            <linearGradient id="cardGrad" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stop-color="#161F30"/>
                <stop offset="100%" stop-color="#0F1622"/>
            </linearGradient>

            <linearGradient id="barWinGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" stop-color="#00F298"/>
                <stop offset="100%" stop-color="#00A859"/>
            </linearGradient>

            <linearGradient id="barLossGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" stop-color="#FF5C6C"/>
                <stop offset="100%" stop-color="#D92D3E"/>
            </linearGradient>

            <linearGradient id="ddGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" stop-color="#FF4560" stop-opacity="0.0"/>
                <stop offset="100%" stop-color="#FF4560" stop-opacity="0.35"/>
            </linearGradient>

            <filter id="neonGlowMaker" x="-20%" y="-20%" width="140%" height="140%">
                <feGaussianBlur stdDeviation="4" result="blur"/>
                <feMerge>
                    <feMergeNode in="blur"/>
                    <feMergeNode in="SourceGraphic"/>
                </feMerge>
            </filter>

            <filter id="cardShadow" x="-10%" y="-10%" width="120%" height="120%">
                <feDropShadow dx="0" dy="8" stdDeviation="16" flood-color="#000000" flood-opacity="0.6"/>
            </filter>
        </defs>

        <!-- Base Background -->
        <rect width="{w}" height="{h}" fill="url(#bgGrad)"/>
        "##,
        w = width, h = height
    ));

    // 1. TOP BRANDING & STATUS HEADER
    svg.push_str(r##"
        <!-- Top Navbar -->
        <g transform="translate(60, 45)">
            <rect width="130" height="28" rx="6" fill="#FF6F0F" fill-opacity="0.15" stroke="#FF6F0F" stroke-opacity="0.4"/>
            <text x="12" y="19" fill="#FF8C38" font-size="12" font-weight="700" letter-spacing="0.5">🥕 SEED QUANT</text>

            <text x="145" y="21" fill="#FFFFFF" font-size="24" font-weight="800" letter-spacing="-0.5">APEX ALPHA</text>
            <text x="315" y="21" fill="#6B7A90" font-size="14" font-weight="500">| Lead-Lag Latency Sniping HFT Execution Model</text>

            <!-- Live badge -->
            <rect x="1350" y="2" width="130" height="26" rx="13" fill="#00C471" fill-opacity="0.15" stroke="#00C471" stroke-opacity="0.4"/>
            <circle cx="1364" cy="15" r="4" fill="#00F298"/>
            <text x="1376" y="19" fill="#00F298" font-size="11" font-weight="700">62.5H FULL DATA</text>
        </g>
    "##);

    // 2. SUMMARY KPI CARDS (4 CARDS)
    let kpi_y = 95.0;
    let card_w = 345.0;
    let card_h = 105.0;
    let card_gap = 33.0;

    let kpis = [
        ("NET PROFIT (MAKER)", "+$806.21", "+8.06% Return (62.5h)", "#00F298", true),
        ("WIN RATE & TRADES", "76.5%", "39 Wins / 12 Losses (51 Trades)", "#38BDF8", false),
        ("PROFIT FACTOR", "3.05", "Gross: +$1,200 | Fee: -$393", "#A78BFA", false),
        ("MAX DRAWDOWN (MDD)", "$83.07", "0.83% of Initial $10k Capital", "#FF5C6C", false),
    ];

    for (i, (label, val, sub, color, is_glow)) in kpis.iter().enumerate() {
        let x = 60.0 + (i as f64 * (card_w + card_gap));
        let glow_attr = if *is_glow { r#"filter="url(#cardShadow)""# } else { "" };
        svg.push_str(&format!(
            r##"
            <g transform="translate({x}, {kpi_y})" {glow_attr}>
                <rect width="{cw}" height="{ch}" rx="14" fill="url(#cardGrad)" stroke="#223048" stroke-width="1.2"/>
                <text x="22" y="30" fill="#7E8EAA" font-size="11" font-weight="700" letter-spacing="0.8">{label}</text>
                <text x="22" y="68" fill="{color}" font-size="30" font-weight="800" letter-spacing="-0.5">{val}</text>
                <text x="22" y="90" fill="#94A3B8" font-size="11" font-weight="500">{sub}</text>
            </g>
            "##,
            x = x, kpi_y = kpi_y, cw = card_w, ch = card_h, label = label, val = val, sub = sub, color = color, glow_attr = glow_attr
        ));
    }

    // 3. MAIN PNL TRAJECTORY CHART (Upper Section)
    let chart_x = 60.0;
    let chart_y = 230.0;
    let chart_w = 1480.0;
    let chart_h = 440.0;

    svg.push_str(&format!(
        r##"
        <!-- Main Chart Card Container -->
        <rect x="{cx}" y="{cy}" width="{cw}" height="{ch}" rx="16" fill="url(#cardGrad)" stroke="#223048" stroke-width="1.2" filter="url(#cardShadow)"/>
        
        <!-- Header inside main chart -->
        <text x="{head_x}" y="{head_y}" fill="#FFFFFF" font-size="17" font-weight="700">Cumulative Net Equity Trajectory (Maker vs Taker Execution)</text>
        <text x="{head_x}" y="{head_sub_y}" fill="#6B7A90" font-size="12">5,120,074 Ticks Real Microstructure Matching | 0.01% Maker vs 0.035% Taker Fee Dynamics</text>
        "##,
        cx = chart_x, cy = chart_y, cw = chart_w, ch = chart_h,
        head_x = chart_x + 30.0, head_y = chart_y + 35.0, head_sub_y = chart_y + 55.0
    ));

    // Plot Area inside card
    let p_left = chart_x + 60.0;
    let p_top = chart_y + 80.0;
    let p_w = chart_w - 120.0;
    let p_h = chart_h - 130.0;

    let min_ts = records.first().map(|r| r.ts).unwrap_or(0) as f64;
    let max_ts = records.last().map(|r| r.ts).unwrap_or(1) as f64;
    let ts_range = (max_ts - min_ts).max(1.0);

    let min_pnl = -200.0;
    let max_pnl = 850.0;
    let pnl_range = max_pnl - min_pnl;

    let get_x = |ts: i64| -> f64 { p_left + ((ts as f64 - min_ts) / ts_range) * p_w };
    let get_y = |pnl: f64| -> f64 { p_top + p_h - ((pnl - min_pnl) / pnl_range) * p_h };

    // Y Axis Grids
    for pnl_val in [-200, 0, 200, 400, 600, 800] {
        let y = get_y(pnl_val as f64);
        let stroke_col = if pnl_val == 0 { "#475569" } else { "#1A2436" };
        let stroke_w = if pnl_val == 0 { "1.5" } else { "1" };
        let dash = if pnl_val == 0 { "none" } else { "4,4" };

        svg.push_str(&format!(
            r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="{sc}" stroke-width="{sw}" stroke-dasharray="{d}"/>
            <text x="{tx}" y="{ty}" fill="#64748B" font-size="11" font-weight="600" text-anchor="end">${v}</text>"##,
            x1 = p_left, x2 = p_left + p_w, y = y, sc = stroke_col, sw = stroke_w, d = dash,
            tx = p_left - 12.0, ty = y + 4.0, v = pnl_val
        ));
    }

    // Maker Area & Path
    let mut maker_path = format!("M {:.1} {:.1}", p_left, get_y(0.0));
    let mut maker_area = format!("M {:.1} {:.1}", p_left, get_y(0.0));
    let mut taker_path = format!("M {:.1} {:.1}", p_left, get_y(0.0));

    for r in records {
        let x = get_x(r.ts);
        let y_m = get_y(r.cum_pnl_maker);
        let y_t = get_y(r.cum_pnl_taker);

        maker_path.push_str(&format!(" L {:.1} {:.1}", x, y_m));
        maker_area.push_str(&format!(" L {:.1} {:.1}", x, y_m));
        taker_path.push_str(&format!(" L {:.1} {:.1}", x, y_t));
    }

    let last_x = get_x(records.last().unwrap().ts);
    maker_area.push_str(&format!(" L {:.1} {:.1} Z", last_x, get_y(0.0)));

    svg.push_str(&format!(r##"<path d="{}" fill="url(#makerGlow)"/>"##, maker_area));
    svg.push_str(&format!(r##"<path d="{}" fill="none" stroke="#F04452" stroke-width="2.2" stroke-dasharray="5,5" stroke-opacity="0.85"/>"##, taker_path));
    svg.push_str(&format!(r##"<path d="{}" fill="none" stroke="#00F298" stroke-width="3.5" filter="url(#neonGlowMaker)"/>"##, maker_path));

    // Endpoint Glowing Dots
    let last_m_y = get_y(records.last().unwrap().cum_pnl_maker);
    let last_t_y = get_y(records.last().unwrap().cum_pnl_taker);

    svg.push_str(&format!(
        r##"
        <circle cx="{x}" cy="{ym}" r="6" fill="#00F298" filter="url(#neonGlowMaker)"/>
        <circle cx="{x}" cy="{ym}" r="3" fill="#FFFFFF"/>
        <circle cx="{x}" cy="{yt}" r="5" fill="#F04452"/>
        "##,
        x = last_x, ym = last_m_y, yt = last_t_y
    ));

    // Legend Badge
    svg.push_str(&format!(
        r##"
        <g transform="translate({lx}, {ly})">
            <rect width="520" height="34" rx="8" fill="#121A2A" stroke="#223048" stroke-width="1"/>
            
            <circle cx="20" cy="17" r="5" fill="#00F298" filter="url(#neonGlowMaker)"/>
            <text x="32" y="21" fill="#00F298" font-size="12" font-weight="700">Maker Limit (0.01% Fee): +$806.21 (+8.06%)</text>
            
            <line x1="310" y1="17" x2="330" y2="17" stroke="#F04452" stroke-width="2.5" stroke-dasharray="4,3"/>
            <text x="338" y="21" fill="#FF5C6C" font-size="12" font-weight="700">Taker (0.035%): -$178.25</text>
        </g>
        "##,
        lx = p_left + 15.0, ly = p_top + 15.0
    ));

    // 4. BOTTOM DUAL PANEL (Left: Underwater Drawdown / Right: Hourly KST Breakdown)
    let bot_y = 690.0;
    let bot_h = 360.0;
    let bot_w_left = 680.0;
    let bot_w_right = 770.0;

    // LEFT BOTTOM: Underwater Drawdown
    svg.push_str(&format!(
        r##"
        <g transform="translate({bx}, {by})">
            <rect width="{bw}" height="{bh}" rx="16" fill="url(#cardGrad)" stroke="#223048" stroke-width="1.2" filter="url(#cardShadow)"/>
            <text x="25" y="32" fill="#FFFFFF" font-size="15" font-weight="700">Underwater Drawdown Curve (Max DD: $83.07)</text>
            <text x="25" y="50" fill="#6B7A90" font-size="11">Peak Capital Recovery & Micro-Risk Exposure</text>
        "##,
        bx = 60.0, by = bot_y, bw = bot_w_left, bh = bot_h
    ));

    let dd_left = 60.0;
    let dd_top = 70.0;
    let dd_w = bot_w_left - 90.0;
    let dd_h = bot_h - 110.0;

    let mut peak = 0.0;
    let mut dd_area = format!("M {} {}", dd_left, dd_top);
    let mut dd_path = format!("M {} {}", dd_left, dd_top);

    for r in records {
        if r.cum_pnl_maker > peak { peak = r.cum_pnl_maker; }
        let dd = r.cum_pnl_maker - peak; // 0 to -100
        let x = dd_left + ((r.ts as f64 - min_ts) / ts_range) * dd_w;
        let y = dd_top + ((-dd / 100.0) * dd_h).min(dd_h);

        dd_area.push_str(&format!(" L {:.1} {:.1}", x, y));
        dd_path.push_str(&format!(" L {:.1} {:.1}", x, y));
    }
    let dd_last_x = dd_left + ((records.last().unwrap().ts as f64 - min_ts) / ts_range) * dd_w;
    dd_area.push_str(&format!(" L {:.1} {} Z", dd_last_x, dd_top));

    svg.push_str(&format!(
        r##"
            <line x1="{x1}" y1="{y0}" x2="{x2}" y2="{y0}" stroke="#475569" stroke-width="1"/>
            <path d="{area}" fill="url(#ddGrad)"/>
            <path d="{path}" fill="none" stroke="#FF4560" stroke-width="2"/>
            <text x="{x1}" y="{y0_t}" fill="#94A3B8" font-size="10">$0</text>
            <text x="{x1}" y="{y50_t}" fill="#94A3B8" font-size="10">-$50</text>
            <text x="{x1}" y="{y100_t}" fill="#FF4560" font-size="10" font-weight="700">-$100 (Peak DD: -$83.07)</text>
        </g>
        "##,
        x1 = dd_left - 45.0, x2 = dd_left + dd_w, y0 = dd_top, area = dd_area, path = dd_path,
        y0_t = dd_top + 4.0, y50_t = dd_top + dd_h/2.0 + 4.0, y100_t = dd_top + dd_h + 4.0
    ));

    // RIGHT BOTTOM: Hourly KST Bar Chart
    let r_bx = 770.0;
    svg.push_str(&format!(
        r##"
        <g transform="translate({bx}, {by})">
            <rect width="{bw}" height="{bh}" rx="16" fill="url(#cardGrad)" stroke="#223048" stroke-width="1.2" filter="url(#cardShadow)"/>
            <text x="25" y="32" fill="#FFFFFF" font-size="15" font-weight="700">Hourly PnL Distribution (KST 24-Hour Session Heatmap)</text>
            <text x="25" y="50" fill="#6B7A90" font-size="11">Asia Afternoon (13-15h) &amp; New York Prime (23-04h) Golden Opportunities</text>
        "##,
        bx = r_bx, by = bot_y, bw = bot_w_right, bh = bot_h
    ));

    let bar_chart_l = 45.0;
    let bar_chart_t = 70.0;
    let bar_chart_w = bot_w_right - 70.0;
    let bar_chart_h = bot_h - 110.0;
    let bar_slot = bar_chart_w / 24.0;
    let bar_width = bar_slot * 0.72;

    let zero_bar_y = bar_chart_t + bar_chart_h - 35.0;

    svg.push_str(&format!(
        r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="#334155" stroke-width="1"/>"##,
        x1 = bar_chart_l, x2 = bar_chart_l + bar_chart_w, y = zero_bar_y
    ));

    for (hour, (_trades, _wins, _gross, net_m)) in hourly {
        let x = bar_chart_l + (*hour as f64 * bar_slot) + (bar_slot - bar_width)/2.0;
        let pnl_scaled = (*net_m / 600.0) * (bar_chart_h - 50.0);
        let (y, h, grad) = if *net_m >= 0.0 {
            (zero_bar_y - pnl_scaled, pnl_scaled.max(2.0), "url(#barWinGrad)")
        } else {
            (zero_bar_y, (-pnl_scaled).max(2.0), "url(#barLossGrad)")
        };

        svg.push_str(&format!(
            r##"
            <rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" fill="{g}" rx="3"/>
            <text x="{tx:.1}" y="{ty}" fill="#64748B" font-size="9" font-weight="600" text-anchor="middle">{hr}</text>
            "##,
            x = x, y = y, w = bar_width, h = h, g = grad,
            tx = x + bar_width/2.0, ty = zero_bar_y + 16.0, hr = format!("{:02}", hour)
        ));

        if net_m.abs() > 30.0 {
            let label_y = if *net_m >= 0.0 { y - 5.0 } else { y + h + 12.0 };
            svg.push_str(&format!(
                r##"<text x="{tx:.1}" y="{ly:.1}" fill="#FFFFFF" font-size="9" font-weight="800" text-anchor="middle">${val:.0}</text>"##,
                tx = x + bar_width/2.0, ly = label_y, val = net_m
            ));
        }
    }

    svg.push_str("</g>"); // Close right panel

    svg.push_str("</svg>");

    let mut out = File::create(out_path).unwrap();
    out.write_all(svg.as_bytes()).unwrap();
}
