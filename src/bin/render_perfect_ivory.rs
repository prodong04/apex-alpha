use chrono::Timelike;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

struct TradeRecord {
    ts: i64,
    dt: String,
    gross_pnl: f64,
    net_pnl_maker: f64,
    cum_pnl_maker: f64,
    net_pnl_taker: f64,
    cum_pnl_taker: f64,
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
                let gross_pnl: f64 = parts[6].parse().unwrap_or(0.0);
                let net_pnl_maker: f64 = parts[8].parse().unwrap_or(0.0);
                let cum_pnl_maker: f64 = parts[9].parse().unwrap_or(0.0);
                let net_pnl_taker: f64 = parts[11].parse().unwrap_or(0.0);
                let cum_pnl_taker: f64 = parts[12].parse().unwrap_or(0.0);

                records.push(TradeRecord {
                    ts, dt, gross_pnl, net_pnl_maker, cum_pnl_maker, net_pnl_taker, cum_pnl_taker,
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

    let svg_path = "/Users/elias/apex-alpha/scratch/ivory_clean.svg";
    generate_perfect_ivory_svg(&records, &hourly_pnl, svg_path);
    println!("✅ Generated Perfect Ivory SVG");
}

fn generate_perfect_ivory_svg(
    records: &[TradeRecord],
    hourly: &BTreeMap<u32, (usize, usize, f64, f64)>,
    out_path: &str,
) {
    let width = 1200.0;
    let height = 1200.0; // Perfect 1:1 Aspect Ratio to prevent any QuickLook crop!

    let mut svg = String::with_capacity(65536);

    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" style="background-color: #FAF8F5; font-family: -apple-system, BlinkMacSystemFont, 'Pretendard', 'Segoe UI', Roboto, sans-serif;">
        <defs>
            <linearGradient id="softBlueGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" stop-color="#3B82F6" stop-opacity="0.25"/>
                <stop offset="60%" stop-color="#60A5FA" stop-opacity="0.08"/>
                <stop offset="100%" stop-color="#93C5FD" stop-opacity="0.0"/>
            </linearGradient>

            <linearGradient id="barGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" stop-color="#2563EB"/>
                <stop offset="100%" stop-color="#60A5FA"/>
            </linearGradient>

            <filter id="softCardShadow" x="-5%" y="-5%" width="110%" height="115%">
                <feDropShadow dx="0" dy="4" stdDeviation="10" flood-color="#0F172A" flood-opacity="0.05"/>
            </filter>
        </defs>

        <!-- Ivory Base -->
        <rect width="{w}" height="{h}" fill="#FAF8F5"/>
        "##,
        w = width, h = height
    ));

    // 1. TOP HEADER
    svg.push_str(r##"
        <g transform="translate(50, 45)">
            <!-- Pill Tag -->
            <rect width="135" height="26" rx="13" fill="#EFF6FF" stroke="#BFDBFE" stroke-width="1"/>
            <circle cx="14" cy="13" r="4" fill="#2563EB"/>
            <text x="24" y="17" fill="#1D4ED8" font-size="11" font-weight="700" letter-spacing="0.5">APEX ALPHA HFT</text>

            <!-- Main Title -->
            <text x="0" y="62" fill="#0F172A" font-size="28" font-weight="800" letter-spacing="-0.5">62.5시간 백테스트 누적 손익 곡선</text>
            <text x="0" y="86" fill="#64748B" font-size="14" font-weight="500">바이낸스 1초 리드래그 충격 감지 ➔ 하이퍼리퀴드 지정가(Post-Only) 메이커 체결 분석 (총 512만 틱)</text>
        </g>
    "##);

    // 2. KPI SUMMARY CARDS (4-Column Layout across width)
    let kpi_y = 155.0;
    let card_w = 260.0;
    let card_h = 75.0;
    let card_gap = 20.0;

    let kpis = [
        ("순수익 (MAKER)", "+$806.21", "+8.06% 수익률", "#2563EB"),
        ("승률 (WIN RATE)", "76.5%", "39승 12패 (총 51회)", "#0F172A"),
        ("프로핏 팩터", "3.05", "Gross: +$1,200 | 수수료: -$393", "#0F172A"),
        ("최대 낙폭 (MDD)", "$83.07", "자본금 대비 단 0.83%", "#E11D48"),
    ];

    for (i, (label, val, sub, col)) in kpis.iter().enumerate() {
        let x = 50.0 + (i as f64 * (card_w + card_gap));
        svg.push_str(&format!(
            r##"
            <g transform="translate({x}, {kpi_y})">
                <rect width="{cw}" height="{ch}" rx="12" fill="#FFFFFF" stroke="#E2E8F0" stroke-width="1" filter="url(#softCardShadow)"/>
                <text x="18" y="24" fill="#64748B" font-size="10.5" font-weight="700" letter-spacing="0.5">{label}</text>
                <text x="18" y="52" fill="{col}" font-size="22" font-weight="800">{val}</text>
                <text x="18" y="68" fill="#94A3B8" font-size="10.5" font-weight="500">{sub}</text>
            </g>
            "##,
            x = x, kpi_y = kpi_y, cw = card_w, ch = card_h, label = label, val = val, sub = sub, col = col
        ));
    }

    // 3. MAIN PNL CHART CARD
    let card_x = 50.0;
    let card_y = 250.0;
    let card_w = 1100.0;
    let card_h = 490.0;

    svg.push_str(&format!(
        r##"
        <rect x="{cx}" y="{cy}" width="{cw}" height="{ch}" rx="16" fill="#FFFFFF" stroke="#E2E8F0" stroke-width="1" filter="url(#softCardShadow)"/>
        "##,
        cx = card_x, cy = card_y, cw = card_w, ch = card_h
    ));

    // Plot inside Main Card
    let p_left = card_x + 65.0;
    let p_top = card_y + 55.0;
    let p_w = card_w - 110.0;
    let p_h = card_h - 110.0;

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
        let is_zero = pnl_val == 0;
        let stroke_col = if is_zero { "#94A3B8" } else { "#F1F5F9" };
        let stroke_w = if is_zero { "1.5" } else { "1" };
        let dash = if is_zero { "none" } else { "4,4" };
        let font_col = if is_zero { "#0F172A" } else { "#64748B" };

        svg.push_str(&format!(
            r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="{sc}" stroke-width="{sw}" stroke-dasharray="{d}"/>
            <text x="{tx}" y="{ty}" fill="{fc}" font-size="12" font-weight="600" text-anchor="end">${v}</text>"##,
            x1 = p_left, x2 = p_left + p_w, y = y, sc = stroke_col, sw = stroke_w, d = dash,
            tx = p_left - 12.0, ty = y + 4.0, fc = font_col, v = pnl_val
        ));
    }

    // Paths
    let zero_y = get_y(0.0);
    let mut maker_path = format!("M {:.1} {:.1}", p_left, zero_y);
    let mut maker_area = format!("M {:.1} {:.1}", p_left, zero_y);
    let mut taker_path = format!("M {:.1} {:.1}", p_left, zero_y);

    for r in records {
        let x = get_x(r.ts);
        let y_m = get_y(r.cum_pnl_maker);
        let y_t = get_y(r.cum_pnl_taker);

        maker_path.push_str(&format!(" L {:.1} {:.1}", x, y_m));
        maker_area.push_str(&format!(" L {:.1} {:.1}", x, y_m));
        taker_path.push_str(&format!(" L {:.1} {:.1}", x, y_t));
    }

    let last_x = get_x(records.last().unwrap().ts);
    maker_area.push_str(&format!(" L {:.1} {:.1} Z", last_x, zero_y));

    // Draw Translucent Pastel Blue Area & Crisp Pastel Blue Line
    svg.push_str(&format!(r##"<path d="{}" fill="url(#softBlueGrad)"/>"##, maker_area));
    svg.push_str(&format!(r##"<path d="{}" fill="none" stroke="#94A3B8" stroke-width="2" stroke-dasharray="5,4"/>"##, taker_path));
    svg.push_str(&format!(r##"<path d="{}" fill="none" stroke="#2563EB" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"/>"##, maker_path));

    // End point dots
    let last_m_y = get_y(records.last().unwrap().cum_pnl_maker);
    let last_t_y = get_y(records.last().unwrap().cum_pnl_taker);

    svg.push_str(&format!(
        r##"
        <circle cx="{x}" cy="{ym}" r="6" fill="#2563EB"/>
        <circle cx="{x}" cy="{ym}" r="3" fill="#FFFFFF"/>
        <circle cx="{x}" cy="{yt}" r="4" fill="#94A3B8"/>
        "##,
        x = last_x, ym = last_m_y, yt = last_t_y
    ));

    // Floating Legend inside Chart
    svg.push_str(&format!(
        r##"
        <g transform="translate({lx}, {ly})">
            <rect width="460" height="32" rx="8" fill="#F8FAFC" stroke="#E2E8F0" stroke-width="1"/>
            
            <line x1="16" y1="16" x2="34" y2="16" stroke="#2563EB" stroke-width="3"/>
            <circle cx="25" cy="16" r="3" fill="#2563EB"/>
            <text x="42" y="20" fill="#1E293B" font-size="12" font-weight="700">지정가 메이커 (0.01%): <tspan fill="#2563EB">+$806.21 (+8.06%)</tspan></text>
            
            <line x1="285" y1="16" x2="303" y2="16" stroke="#94A3B8" stroke-width="2" stroke-dasharray="4,3"/>
            <text x="310" y="20" fill="#64748B" font-size="12" font-weight="600">시장가 테이커: -$178.25</text>
        </g>
        "##,
        lx = p_left + 10.0, ly = p_top + 10.0
    ));

    // Bottom Date Ticks on Main Chart
    for i in 0..=4 {
        let frac = i as f64 / 4.0;
        let ts = min_ts + frac * ts_range;
        let x = p_left + frac * p_w;
        let dt = chrono::DateTime::from_timestamp_millis(ts as i64)
            .map(|d| d.format("%m월 %d일 %H:%M").to_string())
            .unwrap_or_default();

        svg.push_str(&format!(
            r##"<line x1="{x:.1}" y1="{y1}" x2="{x:.1}" y2="{y2}" stroke="#CBD5E1" stroke-width="1"/>
            <text x="{x:.1}" y="{ty}" fill="#64748B" font-size="11" font-weight="600" text-anchor="middle">{dt}</text>"##,
            x = x, y1 = p_top + p_h, y2 = p_top + p_h + 5.0, ty = p_top + p_h + 20.0, dt = dt
        ));
    }

    // 4. BOTTOM HOURLY PNL CARD (KST 24H)
    let bot_x = 50.0;
    let bot_y = 760.0;
    let bot_w = 1100.0;
    let bot_h = 390.0;

    svg.push_str(&format!(
        r##"
        <g transform="translate({bx}, {by})">
            <rect width="{bw}" height="{bh}" rx="16" fill="#FFFFFF" stroke="#E2E8F0" stroke-width="1" filter="url(#softCardShadow)"/>
            
            <text x="30" y="36" fill="#0F172A" font-size="16" font-weight="800">⏰ 한국 시간대별(KST 24H) 누적 손익 분포</text>
            <text x="30" y="56" fill="#64748B" font-size="12" font-weight="500">아시아 오후 피크(13시~15시 / +$647) 및 뉴욕 본장 개장(23시~04시 / +$302)에 수익 80% 집중</text>
        "##,
        bx = bot_x, by = bot_y, bw = bot_w, bh = bot_h
    ));

    let bar_l = 55.0;
    let bar_w_total = bot_w - 100.0;
    let bar_slot = bar_w_total / 24.0;
    let bar_width = bar_slot * 0.65;
    let zero_h_y = 290.0;

    svg.push_str(&format!(
        r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="#CBD5E1" stroke-width="1"/>"##,
        x1 = bar_l - 10.0, x2 = bar_l + bar_w_total + 10.0, y = zero_h_y
    ));

    for (hour, (_trades, _wins, _gross, net_m)) in hourly {
        let x = bar_l + (*hour as f64 * bar_slot) + (bar_slot - bar_width)/2.0;
        let scaled_h = (*net_m / 600.0) * 190.0;

        let (y, h, fill_col) = if *net_m >= 0.0 {
            (zero_h_y - scaled_h, scaled_h.max(2.0), "url(#barGrad)")
        } else {
            (zero_h_y, (-scaled_h).max(2.0), "#CBD5E1")
        };

        svg.push_str(&format!(
            r##"
            <rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" fill="{fc}" rx="3"/>
            <text x="{tx:.1}" y="{ty}" fill="#64748B" font-size="10" font-weight="600" text-anchor="middle">{hr}</text>
            "##,
            x = x, y = y, w = bar_width, h = h, fc = fill_col,
            tx = x + bar_width/2.0, ty = zero_h_y + 18.0, hr = format!("{:02}시", hour)
        ));

        if *net_m > 30.0 {
            svg.push_str(&format!(
                r##"<text x="{tx:.1}" y="{ty:.1}" fill="#1D4ED8" font-size="10.5" font-weight="800" text-anchor="middle">+${v:.0}</text>"##,
                tx = x + bar_width/2.0, ty = y - 5.0, v = net_m
            ));
        }
    }

    svg.push_str("</g>"); // Close bottom card
    svg.push_str("</svg>");

    let mut out = File::create(out_path).unwrap();
    out.write_all(svg.as_bytes()).unwrap();
}
