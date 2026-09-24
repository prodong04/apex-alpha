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

    let svg_path = "/Users/elias/apex-alpha/scratch/ivory_blue_dashboard.svg";
    generate_ivory_blue_svg(&records, &hourly_pnl, svg_path);
    println!("✅ Generated Ivory-Blue SVG");
}

fn generate_ivory_blue_svg(
    records: &[TradeRecord],
    hourly: &BTreeMap<u32, (usize, usize, f64, f64)>,
    out_path: &str,
) {
    let width = 1600.0;
    let height = 1050.0;

    let mut svg = String::with_capacity(65536);

    // SVG Header with Ivory Background and Soft Blue Gradients
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" style="background-color: #FAF8F5; font-family: -apple-system, BlinkMacSystemFont, 'Pretendard', 'SF Pro Display', 'Segoe UI', Roboto, sans-serif;">
        <defs>
            <!-- Translucent Pastel Blue Area Gradient -->
            <linearGradient id="pastelBlueGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" stop-color="#3B82F6" stop-opacity="0.30"/>
                <stop offset="50%" stop-color="#60A5FA" stop-opacity="0.12"/>
                <stop offset="100%" stop-color="#93C5FD" stop-opacity="0.01"/>
            </linearGradient>

            <linearGradient id="hourlyBarGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" stop-color="#3B82F6"/>
                <stop offset="100%" stop-color="#60A5FA"/>
            </linearGradient>

            <filter id="softShadow" x="-5%" y="-5%" width="110%" height="115%">
                <feDropShadow dx="0" dy="4" stdDeviation="12" flood-color="#0F172A" flood-opacity="0.04"/>
            </filter>

            <filter id="cardShadowStrong" x="-5%" y="-5%" width="110%" height="115%">
                <feDropShadow dx="0" dy="6" stdDeviation="16" flood-color="#0F172A" flood-opacity="0.06"/>
            </filter>
        </defs>

        <!-- Ivory Base Background -->
        <rect width="{w}" height="{h}" fill="#FAF8F5"/>
        "##,
        w = width, h = height
    ));

    // 1. HEADER (Clean Minimal Header on Ivory)
    svg.push_str(r##"
        <g transform="translate(70, 55)">
            <!-- Pill Tag -->
            <rect width="145" height="30" rx="15" fill="#EFF6FF" stroke="#BFDBFE" stroke-width="1.2"/>
            <circle cx="16" cy="15" r="4.5" fill="#3B82F6"/>
            <text x="28" y="19.5" fill="#1D4ED8" font-size="12" font-weight="700" letter-spacing="0.4">APEX ALPHA HFT</text>

            <!-- Main Title -->
            <text x="0" y="70" fill="#0F172A" font-size="30" font-weight="800" letter-spacing="-0.8">62.5시간 백테스트 누적 손익 곡선</text>
            <text x="0" y="96" fill="#64748B" font-size="15" font-weight="500">바이낸스 1초 리드래그 충격 감지 ➔ 하이퍼리퀴드 지정가(Post-Only) 메이커 체결 분석 (총 512만 틱)</text>
        </g>
    "##);

    // 2. KPI SUMMARY PILLS (Right Header)
    svg.push_str(r##"
        <g transform="translate(1030, 48)">
            <!-- KPI 1: Net Profit -->
            <g transform="translate(0, 0)">
                <rect width="240" height="74" rx="14" fill="#FFFFFF" stroke="#E2E8F0" stroke-width="1.2" filter="url(#softShadow)"/>
                <text x="20" y="26" fill="#64748B" font-size="11" font-weight="700" letter-spacing="0.5">NET PROFIT (MAKER)</text>
                <text x="20" y="56" fill="#2563EB" font-size="24" font-weight="800">+$806.21</text>
                <text x="145" y="55" fill="#059669" font-size="13" font-weight="700">+8.06%</text>
            </g>

            <!-- KPI 2: Win Rate -->
            <g transform="translate(255, 0)">
                <rect width="245" height="74" rx="14" fill="#FFFFFF" stroke="#E2E8F0" stroke-width="1.2" filter="url(#softShadow)"/>
                <text x="20" y="26" fill="#64748B" font-size="11" font-weight="700" letter-spacing="0.5">WIN RATE &amp; TRADES</text>
                <text x="20" y="56" fill="#0F172A" font-size="24" font-weight="800">76.5%</text>
                <text x="105" y="55" fill="#64748B" font-size="13" font-weight="600">39승 12패</text>
            </g>
        </g>
    "##);

    // 3. MAIN PNL CHART CARD (White Card on Ivory Background)
    let card_x = 70.0;
    let card_y = 175.0;
    let card_w = 1460.0;
    let card_h = 510.0;

    svg.push_str(&format!(
        r##"
        <!-- Main Card Surface -->
        <rect x="{cx}" y="{cy}" width="{cw}" height="{ch}" rx="20" fill="#FFFFFF" stroke="#E2E8F0" stroke-width="1.2" filter="url(#cardShadowStrong)"/>
        "##,
        cx = card_x, cy = card_y, cw = card_w, ch = card_h
    ));

    // Plot inside Main Card
    let p_left = card_x + 70.0;
    let p_top = card_y + 60.0;
    let p_w = card_w - 130.0;
    let p_h = card_h - 120.0;

    let min_ts = records.first().map(|r| r.ts).unwrap_or(0) as f64;
    let max_ts = records.last().map(|r| r.ts).unwrap_or(1) as f64;
    let ts_range = (max_ts - min_ts).max(1.0);

    let min_pnl = -200.0;
    let max_pnl = 850.0;
    let pnl_range = max_pnl - min_pnl;

    let get_x = |ts: i64| -> f64 { p_left + ((ts as f64 - min_ts) / ts_range) * p_w };
    let get_y = |pnl: f64| -> f64 { p_top + p_h - ((pnl - min_pnl) / pnl_range) * p_h };

    // Y Axis Grids & Labels
    for pnl_val in [-200, 0, 200, 400, 600, 800] {
        let y = get_y(pnl_val as f64);
        let is_zero = pnl_val == 0;
        let stroke_col = if is_zero { "#94A3B8" } else { "#F1F5F9" };
        let stroke_w = if is_zero { "1.5" } else { "1" };
        let dash = if is_zero { "none" } else { "4,4" };
        let font_col = if is_zero { "#0F172A" } else { "#64748B" };
        let font_w = if is_zero { "700" } else { "600" };

        svg.push_str(&format!(
            r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="{sc}" stroke-width="{sw}" stroke-dasharray="{d}"/>
            <text x="{tx}" y="{ty}" fill="{fc}" font-size="13" font-weight="{fw}" text-anchor="end">${v}</text>"##,
            x1 = p_left, x2 = p_left + p_w, y = y, sc = stroke_col, sw = stroke_w, d = dash,
            tx = p_left - 15.0, ty = y + 4.5, fc = font_col, fw = font_w, v = pnl_val
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
    svg.push_str(&format!(r##"<path d="{}" fill="url(#pastelBlueGrad)"/>"##, maker_area));
    svg.push_str(&format!(r##"<path d="{}" fill="none" stroke="#94A3B8" stroke-width="2.2" stroke-dasharray="6,5"/>"##, taker_path));
    svg.push_str(&format!(r##"<path d="{}" fill="none" stroke="#2563EB" stroke-width="3.8" stroke-linecap="round" stroke-linejoin="round"/>"##, maker_path));

    // End point dots & badges
    let last_m_y = get_y(records.last().unwrap().cum_pnl_maker);
    let last_t_y = get_y(records.last().unwrap().cum_pnl_taker);

    svg.push_str(&format!(
        r##"
        <circle cx="{x}" cy="{ym}" r="7" fill="#2563EB"/>
        <circle cx="{x}" cy="{ym}" r="4" fill="#FFFFFF"/>
        <circle cx="{x}" cy="{yt}" r="5" fill="#94A3B8"/>
        "##,
        x = last_x, ym = last_m_y, yt = last_t_y
    ));

    // Top Floating Legend Box inside Chart
    svg.push_str(&format!(
        r##"
        <g transform="translate({lx}, {ly})">
            <rect width="520" height="38" rx="10" fill="#F8FAFC" stroke="#E2E8F0" stroke-width="1.2"/>
            
            <!-- Maker Legend -->
            <line x1="20" y1="19" x2="42" y2="19" stroke="#2563EB" stroke-width="3.5"/>
            <circle cx="31" cy="19" r="4" fill="#2563EB"/>
            <text x="50" y="24" fill="#1E293B" font-size="13" font-weight="700">지정가 메이커 (0.01%): <tspan fill="#2563EB">+$806.21 (+8.06%)</tspan></text>
            
            <!-- Taker Legend -->
            <line x1="330" y1="19" x2="352" y2="19" stroke="#94A3B8" stroke-width="2.2" stroke-dasharray="4,3"/>
            <text x="360" y="24" fill="#64748B" font-size="13" font-weight="600">시장가 테이커: -$178.25</text>
        </g>
        "##,
        lx = p_left + 15.0, ly = p_top + 15.0
    ));

    // Bottom Date Ticks on Main Chart
    for i in 0..=5 {
        let frac = i as f64 / 5.0;
        let ts = min_ts + frac * ts_range;
        let x = p_left + frac * p_w;
        let dt = chrono::DateTime::from_timestamp_millis(ts as i64)
            .map(|d| d.format("%m월 %d일 %H:%M").to_string())
            .unwrap_or_default();

        svg.push_str(&format!(
            r##"<line x1="{x:.1}" y1="{y1}" x2="{x:.1}" y2="{y2}" stroke="#CBD5E1" stroke-width="1.2"/>
            <text x="{x:.1}" y="{ty}" fill="#64748B" font-size="12" font-weight="600" text-anchor="middle">{dt}</text>"##,
            x = x, y1 = p_top + p_h, y2 = p_top + p_h + 6.0, ty = p_top + p_h + 24.0, dt = dt
        ));
    }

    // 4. BOTTOM HOURLY PNL CARD (Clean White Card on Ivory)
    let bot_x = 70.0;
    let bot_y = 715.0;
    let bot_w = 1460.0;
    let bot_h = 280.0;

    svg.push_str(&format!(
        r##"
        <g transform="translate({bx}, {by})">
            <!-- Card Container -->
            <rect width="{bw}" height="{bh}" rx="20" fill="#FFFFFF" stroke="#E2E8F0" stroke-width="1.2" filter="url(#cardShadowStrong)"/>
            
            <!-- Card Title -->
            <text x="35" y="38" fill="#0F172A" font-size="16" font-weight="800">⏰ 한국 시간대별(KST 24H) 누적 손익 분포</text>
            <text x="35" y="58" fill="#64748B" font-size="12" font-weight="500">아시아 오후 피크(13시~15시 / +$647) 및 뉴욕 본장 개장(23시~04시 / +$302)에 수익 집중</text>
        "##,
        bx = bot_x, by = bot_y, bw = bot_w, bh = bot_h
    ));

    let bar_l = 65.0;
    let bar_w_total = bot_w - 130.0;
    let bar_slot = bar_w_total / 24.0;
    let bar_width = bar_slot * 0.68;
    let zero_h_y = 205.0;

    svg.push_str(&format!(
        r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="#CBD5E1" stroke-width="1.2"/>"##,
        x1 = bar_l - 10.0, x2 = bar_l + bar_w_total + 10.0, y = zero_h_y
    ));

    for (hour, (_trades, _wins, _gross, net_m)) in hourly {
        let x = bar_l + (*hour as f64 * bar_slot) + (bar_slot - bar_width)/2.0;
        let scaled_h = (*net_m / 600.0) * 115.0;

        let (y, h, fill_col) = if *net_m >= 0.0 {
            (zero_h_y - scaled_h, scaled_h.max(2.0), "url(#hourlyBarGrad)")
        } else {
            (zero_h_y, (-scaled_h).max(2.0), "#CBD5E1")
        };

        svg.push_str(&format!(
            r##"
            <rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" fill="{fc}" rx="4"/>
            <text x="{tx:.1}" y="{ty}" fill="#64748B" font-size="11" font-weight="600" text-anchor="middle">{hr}</text>
            "##,
            x = x, y = y, w = bar_width, h = h, fc = fill_col,
            tx = x + bar_width/2.0, ty = zero_h_y + 20.0, hr = format!("{:02}시", hour)
        ));

        if *net_m > 30.0 {
            svg.push_str(&format!(
                r##"<text x="{tx:.1}" y="{ty:.1}" fill="#1D4ED8" font-size="11" font-weight="800" text-anchor="middle">+${v:.0}</text>"##,
                tx = x + bar_width/2.0, ty = y - 6.0, v = net_m
            ));
        }
    }

    svg.push_str("</g>"); // Close bottom card
    svg.push_str("</svg>");

    let mut out = File::create(out_path).unwrap();
    out.write_all(svg.as_bytes()).unwrap();
}
