use chrono::Timelike;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

fn main() {
    let file = File::open("/Users/elias/apex-alpha/scratch/backtest_pnl_timeseries.csv").unwrap();
    let reader = BufReader::new(file);

    let mut hourly_pnl: BTreeMap<u32, (usize, usize, f64, f64)> = BTreeMap::new();
    for i in 0..24 {
        hourly_pnl.insert(i, (0, 0, 0.0, 0.0));
    }

    for (idx, line) in reader.lines().enumerate() {
        if idx == 0 { continue; }
        if let Ok(l) = line {
            let parts: Vec<&str> = l.split(',').collect();
            if parts.len() >= 10 {
                let ts: i64 = parts[0].parse().unwrap_or(0);
                let gross: f64 = parts[6].parse().unwrap_or(0.0);
                let net_m: f64 = parts[8].parse().unwrap_or(0.0);

                if let Some(dt) = chrono::DateTime::from_timestamp_millis(ts) {
                    let hour_kst = (dt.hour() + 9) % 24;
                    let entry = hourly_pnl.entry(hour_kst).or_insert((0, 0, 0.0, 0.0));
                    entry.0 += 1;
                    if net_m > 0.0 { entry.1 += 1; }
                    entry.2 += gross;
                    entry.3 += net_m;
                }
            }
        }
    }

    let width = 1100.0;
    let height = 550.0;
    let pad_left = 70.0;
    let pad_right = 40.0;
    let pad_top = 70.0;
    let pad_bottom = 60.0;
    let chart_w = width - pad_left - pad_right;
    let chart_h = height - pad_top - pad_bottom;

    let max_val = 600.0;
    let min_val = -50.0;
    let val_range = max_val - min_val;

    let get_y = |v: f64| -> f64 { pad_top + chart_h - ((v - min_val) / val_range) * chart_h };

    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="100%" height="100%" style="background-color: #090C10; font-family: -apple-system, sans-serif;">
        <text x="{}" y="35" fill="#FFFFFF" font-size="20" font-weight="bold">⏰ 한국 시간(KST 24H) 시간대별 누적 손익 분포</text>
        <text x="{}" y="55" fill="#8B95A1" font-size="12">글로벌 금융 세션별 스파이크 발생 빈도 및 메이커 순수익</text>
        <rect x="{}" y="{}" width="{}" height="{}" fill="#161A22" rx="8" stroke="#30363D"/>
        "##,
        width, height, pad_left, pad_left, pad_left, pad_top, chart_w, chart_h
    );

    // Zero line
    let zero_y = get_y(0.0);
    svg.push_str(&format!(r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#64748B" stroke-width="1.5"/>"##, pad_left, zero_y, pad_left + chart_w, zero_y));

    // Y Grid
    for v in [0, 150, 300, 450, 600] {
        let y = get_y(v as f64);
        svg.push_str(&format!(
            r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#21262D" stroke-dasharray="3,3"/>
            <text x="{}" y="{}" fill="#8B95A1" font-size="11" text-anchor="end">${}</text>"##,
            pad_left, y, pad_left + chart_w, y, pad_left - 10.0, y + 4.0, v
        ));
    }

    let bar_slot = chart_w / 24.0;
    let bar_w = bar_slot * 0.65;

    for (hour, (_trades, _wins, _gross, net_m)) in hourly_pnl {
        let x = pad_left + (hour as f64 * bar_slot) + (bar_slot - bar_w) / 2.0;
        let y = if net_m >= 0.0 { get_y(net_m) } else { zero_y };
        let h = if net_m >= 0.0 { zero_y - get_y(net_m) } else { get_y(net_m) - zero_y };
        let color = if net_m >= 0.0 { "#00C471" } else { "#F04452" };

        if h > 1.0 {
            svg.push_str(&format!(r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{}" rx="3"/>"##, x, y, bar_w, h, color));
            if net_m.abs() > 20.0 {
                let text_y = if net_m >= 0.0 { y - 6.0 } else { y + h + 12.0 };
                svg.push_str(&format!(r##"<text x="{:.1}" y="{:.1}" fill="#FFFFFF" font-size="10" font-weight="bold" text-anchor="middle">${:.0}</text>"##, x + bar_w/2.0, text_y, net_m));
            }
        }

        // X label
        svg.push_str(&format!(r##"<text x="{:.1}" y="{}" fill="#8B95A1" font-size="10" text-anchor="middle">{:02}시</text>"##, x + bar_w/2.0, pad_top + chart_h + 18.0, hour));
    }

    svg.push_str("</svg>");

    let out_path = "/Users/elias/apex-alpha/scratch/hourly_pnl.svg";
    let mut out = File::create(out_path).unwrap();
    out.write_all(svg.as_bytes()).unwrap();
    println!("Generated hourly SVG at {}", out_path);
}
