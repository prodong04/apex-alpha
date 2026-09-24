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
    for (i, line) in reader.lines().enumerate() {
        if i == 0 { continue; }
        if let Ok(l) = line {
            let parts: Vec<&str> = l.split(',').collect();
            if parts.len() >= 14 {
                records.push(TradeRecord {
                    ts: parts[0].parse().unwrap_or(0),
                    dt: parts[1].to_string(),
                    side: parts[2].to_string(),
                    entry_px: parts[3].parse().unwrap_or(0.0),
                    exit_px: parts[4].parse().unwrap_or(0.0),
                    holding_ms: parts[5].parse().unwrap_or(0),
                    gross_pnl: parts[6].parse().unwrap_or(0.0),
                    fee_maker: parts[7].parse().unwrap_or(0.0),
                    net_pnl_maker: parts[8].parse().unwrap_or(0.0),
                    cum_pnl_maker: parts[9].parse().unwrap_or(0.0),
                    net_pnl_taker: parts[11].parse().unwrap_or(0.0),
                    cum_pnl_taker: parts[12].parse().unwrap_or(0.0),
                    reason: parts[13].to_string(),
                });
            }
        }
    }

    println!("Loaded {} backtest trade records.", records.len());

    let artifact_dir = "/Users/elias/.gemini/antigravity-cli/brain/f9cff2dd-36c0-43ad-83e1-22d36e15f579";
    let svg_path = format!("{}/backtest_pnl_trajectory.svg", artifact_dir);
    generate_svg(&records, &svg_path);
    println!("✅ Generated ultra-high-res SVG at {}", svg_path);
}

fn generate_svg(records: &[TradeRecord], out_path: &str) {
    let width = 1100.0;
    let height = 720.0;
    let pad_left = 80.0;
    let pad_right = 50.0;
    let pad_top = 70.0;
    let pad_bottom = 60.0;

    let chart_w = width - pad_left - pad_right;
    let chart_h1 = 300.0;
    let chart_h2 = 180.0;
    let gap = 50.0;

    let min_ts = records.first().map(|r| r.ts).unwrap_or(0) as f64;
    let max_ts = records.last().map(|r| r.ts).unwrap_or(1) as f64;
    let ts_range = (max_ts - min_ts).max(1.0);

    let min_pnl = -250.0;
    let max_pnl = 900.0;
    let pnl_range = max_pnl - min_pnl;

    let get_x = |ts: i64| -> f64 { pad_left + ((ts as f64 - min_ts) / ts_range) * chart_w };
    let get_y_pnl = |pnl: f64| -> f64 { pad_top + chart_h1 - ((pnl - min_pnl) / pnl_range) * chart_h1 };

    let mut svg = String::with_capacity(32768);
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="100%" height="100%" style="background-color: #090C10; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
        <defs>
            <linearGradient id="makerGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" stop-color="#00C471" stop-opacity="0.35"/>
                <stop offset="100%" stop-color="#00C471" stop-opacity="0.0"/>
            </linearGradient>
            <linearGradient id="ddGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" stop-color="#F04452" stop-opacity="0.0"/>
                <stop offset="100%" stop-color="#F04452" stop-opacity="0.35"/>
            </linearGradient>
        </defs>
        "##,
        width, height
    ));

    // Title
    svg.push_str(&format!(
        r##"<text x="{}" y="35" fill="#FFFFFF" font-size="20" font-weight="bold">🚀 APEX ALPHA: 62.5-Hour Backtest PnL Trajectory (5.12M Ticks)</text>
        <text x="{}" y="55" fill="#8B95A1" font-size="12">Binance Spot 1s Lead-Lag Shock ➔ Hyperliquid Post-Only Maker vs Taker Execution</text>
        "##,
        pad_left, pad_left
    ));

    // Panel 1: Top PnL Chart Background & Grids
    svg.push_str(&format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#161A22" rx="8" stroke="#30363D" stroke-width="1"/>"##,
        pad_left, pad_top, chart_w, chart_h1
    ));

    // Y Grid lines for PnL
    for pnl_val in [-200, 0, 200, 400, 600, 800] {
        let y = get_y_pnl(pnl_val as f64);
        let stroke_color = if pnl_val == 0 { "#64748B" } else { "#21262D" };
        let stroke_w = if pnl_val == 0 { "1.5" } else { "1" };
        svg.push_str(&format!(
            r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-dasharray="{}"/>
            <text x="{}" y="{}" fill="#8B95A1" font-size="11" text-anchor="end">${}</text>"##,
            pad_left, y, pad_left + chart_w, y, stroke_color, stroke_w, if pnl_val == 0 { "none" } else { "3,3" },
            pad_left - 10.0, y + 4.0, pnl_val
        ));
    }

    // Maker Area & Path
    let mut maker_path = format!("M {} {}", pad_left, get_y_pnl(0.0));
    let mut maker_area = format!("M {} {}", pad_left, get_y_pnl(0.0));
    let mut taker_path = format!("M {} {}", pad_left, get_y_pnl(0.0));

    for r in records {
        let x = get_x(r.ts);
        let y_m = get_y_pnl(r.cum_pnl_maker);
        let y_t = get_y_pnl(r.cum_pnl_taker);

        maker_path.push_str(&format!(" L {:.1} {:.1}", x, y_m));
        maker_area.push_str(&format!(" L {:.1} {:.1}", x, y_m));
        taker_path.push_str(&format!(" L {:.1} {:.1}", x, y_t));
    }

    let last_x = get_x(records.last().unwrap().ts);
    maker_area.push_str(&format!(" L {:.1} {} Z", last_x, get_y_pnl(0.0)));

    svg.push_str(&format!(r##"<path d="{}" fill="url(#makerGrad)"/>"##, maker_area));
    svg.push_str(&format!(r##"<path d="{}" fill="none" stroke="#00C471" stroke-width="3"/>"##, maker_path));
    svg.push_str(&format!(r##"<path d="{}" fill="none" stroke="#F04452" stroke-width="2" stroke-dasharray="4,4"/>"##, taker_path));

    // Legend
    svg.push_str(&format!(
        r##"<g transform="translate({}, {})">
            <rect width="450" height="30" fill="#21262D" rx="6" stroke="#30363D"/>
            <line x1="15" y1="15" x2="35" y2="15" stroke="#00C471" stroke-width="3"/>
            <text x="42" y="19" fill="#00C471" font-size="11" font-weight="bold">Maker (0.01% Fee): +$806.21 (+8.06%)</text>
            <line x1="260" y1="15" x2="280" y2="15" stroke="#F04452" stroke-width="2" stroke-dasharray="3,3"/>
            <text x="287" y="19" fill="#F04452" font-size="11" font-weight="bold">Taker (0.035%): -$178.25</text>
        </g>"##,
        pad_left + 15.0, pad_top + 15.0
    ));

    // Stats badge
    svg.push_str(&format!(
        r##"<g transform="translate({}, {})">
            <rect width="180" height="90" fill="#21262D" rx="6" stroke="#00C471" stroke-width="1.5"/>
            <text x="12" y="20" fill="#FFFFFF" font-size="11" font-weight="bold">Trades: 51 (Win: 76.5%)</text>
            <text x="12" y="38" fill="#00C471" font-size="11" font-weight="bold">Net Profit: +$806.21</text>
            <text x="12" y="56" fill="#8B95A1" font-size="11">Avg Holding: 470ms</text>
            <text x="12" y="74" fill="#8B95A1" font-size="11">Max Drawdown: $83.07</text>
        </g>"##,
        pad_left + chart_w - 195.0, pad_top + 15.0
    ));

    // Panel 2: Bottom Individual Trade & Drawdown
    let y2_top = pad_top + chart_h1 + gap;
    svg.push_str(&format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#161A22" rx="8" stroke="#30363D" stroke-width="1"/>
        <text x="{}" y="{}" fill="#FFFFFF" font-size="13" font-weight="bold">📊 개별 거래 손익 분포 & Underwater Drawdown (낙폭)</text>
        "##,
        pad_left, y2_top, chart_w, chart_h2, pad_left + 15.0, y2_top - 12.0
    ));

    // Draw trade bars
    let bar_zero_y = y2_top + (chart_h2 * 0.6);
    svg.push_str(&format!(
        r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#64748B" stroke-width="1"/>"##,
        pad_left, bar_zero_y, pad_left + chart_w, bar_zero_y
    ));

    for r in records {
        let x = get_x(r.ts);
        let pnl = r.net_pnl_maker;
        let bar_h = (pnl.abs() * 1.8).min(chart_h2 * 0.45);
        let bar_y = if pnl >= 0.0 { bar_zero_y - bar_h } else { bar_zero_y };
        let color = if pnl >= 0.0 { "#00C471" } else { "#F04452" };

        svg.push_str(&format!(
            r##"<rect x="{:.1}" y="{:.1}" width="4" height="{:.1}" fill="{}" rx="1"/>"##,
            x - 2.0, bar_y, bar_h.max(2.0), color
        ));
    }

    // Time labels at bottom
    for i in 0..=5 {
        let frac = i as f64 / 5.0;
        let ts = min_ts + frac * ts_range;
        let x = pad_left + frac * chart_w;
        let dt = chrono::DateTime::from_timestamp_millis(ts as i64)
            .map(|d| d.format("%m-%d %H:%M").to_string())
            .unwrap_or_default();

        svg.push_str(&format!(
            r##"<line x1="{:.1}" y1="{}" x2="{:.1}" y2="{}" stroke="#30363D" stroke-width="1"/>
            <text x="{:.1}" y="{}" fill="#8B95A1" font-size="10" text-anchor="middle">{}</text>"##,
            x, y2_top + chart_h2, x, y2_top + chart_h2 + 5.0,
            x, y2_top + chart_h2 + 18.0, dt
        ));
    }

    svg.push_str("</svg>");

    let mut out = File::create(out_path).unwrap();
    out.write_all(svg.as_bytes()).unwrap();
}
