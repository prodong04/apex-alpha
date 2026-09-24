// ==============================================================================
// Daangn Seed Design - Real-Time HFT Execution Engine Frontend
// ==============================================================================

const MAX_TICKS = 800;
const MAX_BUBBLES = 200;

const spotTicks = [];
const spotBubbles = [];
let spotPrice = 0;

const hlTicks = [];
const hlBubbles = [];
let hlPrice = 0;
let hlBestBid = 0;
let hlBestAsk = 0;

let canvasSpot = null, ctxSpot = null;
let canvasHl = null, ctxHl = null;

let pnlSeries = null;
let chartPnl = null;
let lastPnlTime = 0;

function initCanvases() {
  canvasSpot = document.getElementById('canvas-spot');
  if (canvasSpot) ctxSpot = canvasSpot.getContext('2d');

  canvasHl = document.getElementById('canvas-hl');
  if (canvasHl) ctxHl = canvasHl.getContext('2d');

  function resizeAll() {
    [canvasSpot, canvasHl].forEach(c => {
      if (!c) return;
      const rect = c.parentElement.getBoundingClientRect();
      c.width = rect.width * window.devicePixelRatio;
      c.height = rect.height * window.devicePixelRatio;
    });
  }

  resizeAll();
  window.addEventListener('resize', resizeAll);

  function renderAll() {
    const now = Date.now();
    drawExchangeChart(canvasSpot, ctxSpot, spotTicks, spotBubbles, spotPrice, '#FFAA00', 'Binance Spot', now);
    drawExchangeChart(canvasHl, ctxHl, hlTicks, hlBubbles, hlPrice, '#38BDF8', 'Hyperliquid', now);
    requestAnimationFrame(renderAll);
  }
  renderAll();
}

function drawExchangeChart(canvas, ctx, ticks, bubbles, latestPrice, themeColor, exchangeName, now) {
  if (!canvas || !ctx) return;

  const dpr = window.devicePixelRatio || 1;
  const width = canvas.width / dpr;
  const height = canvas.height / dpr;

  ctx.save();
  ctx.scale(dpr, dpr);

  ctx.fillStyle = '#090C10';
  ctx.fillRect(0, 0, width, height);

  const timeWindowMs = 60000;
  const minTime = now - timeWindowMs;

  let minPx = Infinity;
  let maxPx = -Infinity;

  for (let i = 0; i < ticks.length; i++) {
    const t = ticks[i];
    if (t.time >= minTime) {
      if (t.price < minPx) minPx = t.price;
      if (t.price > maxPx) maxPx = t.price;
    }
  }

  for (let i = 0; i < bubbles.length; i++) {
    const b = bubbles[i];
    if (b.time >= minTime) {
      if (b.price < minPx) minPx = b.price;
      if (b.price > maxPx) maxPx = b.price;
    }
  }

  if (minPx === Infinity || maxPx === -Infinity || minPx === maxPx) {
    const base = latestPrice > 0 ? latestPrice : 77000;
    minPx = base - 10;
    maxPx = base + 10;
  }

  const margin = (maxPx - minPx) * 0.15 || 5;
  minPx -= margin;
  maxPx += margin;
  const priceRange = maxPx - minPx;

  const chartHeight = height * 0.82;
  const volHeight = height * 0.18;

  function getY(p) { return chartHeight - ((p - minPx) / priceRange) * chartHeight; }
  function getX(t) { return ((t - minTime) / timeWindowMs) * (width - 60); }

  // Grid lines
  ctx.strokeStyle = 'rgba(240, 246, 252, 0.04)';
  ctx.lineWidth = 1;
  for (let i = 0; i <= 4; i++) {
    const y = (chartHeight / 4) * i;
    const pxVal = maxPx - (priceRange / 4) * i;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(width - 60, y);
    ctx.stroke();

    ctx.fillStyle = '#64748B';
    ctx.font = '10px -apple-system, sans-serif';
    ctx.fillText(`$${pxVal.toFixed(1)}`, width - 56, y + 3.5);
  }

  // Volume Area Divider
  ctx.strokeStyle = 'rgba(240, 246, 252, 0.08)';
  ctx.beginPath();
  ctx.moveTo(0, chartHeight);
  ctx.lineTo(width, chartHeight);
  ctx.stroke();

  // Draw Tick Line
  if (ticks.length > 0) {
    ctx.strokeStyle = themeColor;
    ctx.lineWidth = 1.8;
    ctx.beginPath();
    let first = true;

    for (let i = 0; i < ticks.length; i++) {
      const t = ticks[i];
      if (t.time >= minTime) {
        const x = getX(t.time);
        const y = getY(t.price);
        if (first) { ctx.moveTo(x, y); first = false; }
        else { ctx.lineTo(x, y); }
      }
    }

    if (!first && latestPrice > 0) {
      ctx.lineTo(width - 60, getY(latestPrice));
    }
    ctx.stroke();
  }

  // Draw Trade Bubbles
  for (let i = 0; i < bubbles.length; i++) {
    const b = bubbles[i];
    if (b.time < minTime) continue;

    const x = getX(b.time);
    const y = getY(b.price);

    ctx.save();
    ctx.beginPath();
    ctx.arc(x, y, b.radius, 0, Math.PI * 2);

    if (b.side === 'BUY') {
      ctx.fillStyle = 'rgba(0, 196, 113, 0.75)';
      ctx.shadowColor = '#00C471';
      ctx.shadowBlur = 6;
    } else {
      ctx.fillStyle = 'rgba(240, 68, 82, 0.75)';
      ctx.shadowColor = '#F04452';
      ctx.shadowBlur = 6;
    }
    ctx.fill();
    ctx.lineWidth = 1.5;
    ctx.strokeStyle = themeColor;
    ctx.stroke();
    ctx.restore();

    // Volume Spike Bar
    const barH = Math.min(volHeight - 8, Math.sqrt(b.qty) * 10);
    ctx.fillStyle = b.side === 'BUY' ? 'rgba(0, 196, 113, 0.6)' : 'rgba(240, 68, 82, 0.6)';
    ctx.fillRect(x - 1, height - barH, 2.5, barH);
  }

  // Head Price Tag
  if (latestPrice > 0) {
    const curY = getY(latestPrice);
    ctx.fillStyle = themeColor;
    ctx.fillRect(width - 58, curY - 9, 56, 18);
    ctx.fillStyle = '#090C10';
    ctx.font = 'bold 10px -apple-system, sans-serif';
    ctx.fillText(`$${latestPrice.toFixed(1)}`, width - 55, curY + 3.5);

    ctx.strokeStyle = themeColor;
    ctx.setLineDash([2, 2]);
    ctx.beginPath();
    ctx.moveTo(0, curY);
    ctx.lineTo(width - 58, curY);
    ctx.stroke();
  }

  ctx.restore();
}

function initPnlChart() {
  try {
    const pnlContainer = document.getElementById('chart-pnl');
    if (!pnlContainer || typeof LightweightCharts === 'undefined') return;

    chartPnl = LightweightCharts.createChart(pnlContainer, {
      layout: { background: { color: '#161A22' }, textColor: '#8B95A1' },
      grid: { vertLines: { color: 'rgba(240, 246, 252, 0.04)' }, horzLines: { color: 'rgba(240, 246, 252, 0.04)' } },
      timeScale: { timeVisible: true, secondsVisible: true, borderColor: 'rgba(240, 246, 252, 0.08)' },
      width: pnlContainer.clientWidth || 450,
      height: 230,
    });

    pnlSeries = chartPnl.addAreaSeries({
      topColor: 'rgba(0, 196, 113, 0.35)',
      bottomColor: 'rgba(0, 196, 113, 0.02)',
      lineColor: '#00C471',
      lineWidth: 2,
    });

    window.addEventListener('resize', () => {
      if (chartPnl && pnlContainer) chartPnl.applyOptions({ width: pnlContainer.clientWidth });
    });
  } catch (e) {
    console.error('PnL chart init error:', e);
  }
}

function connectWebSocket() {
  const host = window.location.host || '127.0.0.1:3000';
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const ws = new WebSocket(`${protocol}//${host}/ws`);
  const statusEl = document.getElementById('conn-status');

  ws.onopen = () => {
    if (statusEl) statusEl.innerText = '실시간 HFT 엔진 가동 중';
  };

  ws.onmessage = (event) => {
    try {
      const payload = JSON.parse(event.data);
      handleUiEvent(payload);
    } catch (e) {
      console.error('WS parse error:', e);
    }
  };

  ws.onclose = () => {
    if (statusEl) statusEl.innerText = '재접속 시도 중...';
    setTimeout(connectWebSocket, 1000);
  };
}

function handleUiEvent(event) {
  const { type, data } = event;

  if (type === 'exchange_trade') {
    const bubbleR = Math.min(28, Math.max(3.5, Math.sqrt(data.qty) * 9));
    const bubbleItem = { time: data.timestamp, price: data.price, qty: data.qty, side: data.side, radius: bubbleR };

    if (data.exchange.includes('Spot')) {
      spotPrice = data.price;
      spotTicks.push({ time: data.timestamp, price: data.price });
      if (spotTicks.length > MAX_TICKS) spotTicks.shift();
      spotBubbles.push(bubbleItem);
      if (spotBubbles.length > MAX_BUBBLES) spotBubbles.shift();

      const spotEl = document.getElementById('val-spot-price');
      if (spotEl) spotEl.innerText = `$${data.price.toLocaleString(undefined, { minimumFractionDigits: 2 })}`;
      const panelSpotEl = document.getElementById('panel-spot-px');
      if (panelSpotEl) panelSpotEl.innerText = `$${data.price.toFixed(1)}`;
    } else if (data.exchange.includes('Hyperliquid')) {
      hlPrice = data.price;
      hlTicks.push({ time: data.timestamp, price: data.price });
      if (hlTicks.length > MAX_TICKS) hlTicks.shift();
      hlBubbles.push(bubbleItem);
      if (hlBubbles.length > MAX_BUBBLES) hlBubbles.shift();

      const hlEl = document.getElementById('val-hl-price');
      if (hlEl) hlEl.innerText = `$${data.price.toLocaleString(undefined, { minimumFractionDigits: 2 })}`;
      const panelHlEl = document.getElementById('panel-hl-px');
      if (panelHlEl) panelHlEl.innerText = `$${data.price.toFixed(1)}`;
    }
  }
  else if (type === 'market_tick') {
    hlPrice = data.price;
    hlBestBid = data.best_bid;
    hlBestAsk = data.best_ask;
    const bboEl = document.getElementById('val-hl-bbo');
    if (bboEl) bboEl.innerText = `Best Bid: $${data.best_bid.toFixed(1)} | Ask: $${data.best_ask.toFixed(1)}`;
  }
  else if (type === 'order_placed') {
    addOrderTableRow(data.timestamp, data.order_id, data.side, data.price, data.qty, '대기 중 (500ms 만료 대기)', 'badge-pending');
  }
  else if (type === 'order_fill') {
    const pnlSign = data.realized_pnl >= 0 ? '+' : '';
    const desc = `🎯 메이커 체결! (수수료: -$${data.fee.toFixed(4)}${data.realized_pnl !== 0 ? ` | 실현손익: ${pnlSign}$${data.realized_pnl.toFixed(2)}` : ''})`;
    addOrderTableRow(data.timestamp, data.order_id, data.side, data.price, data.qty, desc, 'badge-buy');
  }
  else if (type === 'order_canceled') {
    addOrderTableRow(data.timestamp, data.order_id, '-', 0, 0, `⏰ 자동 취소: ${data.reason}`, 'badge-hold');
  }
  else if (type === 'account_update') {
    const isProfit = data.net_pnl >= 0;
    const equityEl = document.getElementById('val-equity');
    if (equityEl) {
      equityEl.innerText = `$${data.equity.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
      equityEl.className = `card-value ${isProfit ? 'text-positive' : 'text-negative'}`;
    }

    const pnlSign = isProfit ? '+' : '';
    const pnlPct = ((data.equity - 10000.0) / 10000.0) * 100.0;
    const netPnlEl = document.getElementById('val-net-pnl');
    if (netPnlEl) netPnlEl.innerText = `순손익: ${pnlSign}$${data.net_pnl.toFixed(2)} (${pnlSign}${pnlPct.toFixed(2)}%) | 승률: ${data.win_rate.toFixed(1)}%`;

    const posSizeEl = document.getElementById('val-pos-size');
    const posEntryEl = document.getElementById('val-pos-entry');
    if (posSizeEl && posEntryEl) {
      if (data.position_side && data.position_size > 0.00001) {
        const sideText = data.position_side === 'Buy' ? 'LONG (매수)' : 'SHORT (매도)';
        const sideClass = data.position_side === 'Buy' ? 'text-positive' : 'text-negative';
        posSizeEl.innerHTML = `<span class="${sideClass}">${sideText} ${data.position_size.toFixed(4)} BTC</span>`;
        posEntryEl.innerText = `진입가: $${data.entry_price.toFixed(2)} | ROE: ${data.roe_pct >= 0 ? '+' : ''}${data.roe_pct.toFixed(2)}%`;
      } else {
        posSizeEl.innerText = '무포지션 (0.00 BTC)';
        posEntryEl.innerText = '재고 정상 (델타 0.0)';
      }
    }

    const feeEl = document.getElementById('val-total-fees');
    if (feeEl) feeEl.innerText = `누적 메이커 수수료: -$${data.total_fees.toFixed(4)}`;

    let timeSec = Math.floor(data.timestamp / 1000);
    if (timeSec <= lastPnlTime) timeSec = lastPnlTime + 1;
    lastPnlTime = timeSec;

    if (pnlSeries) {
      try { pnlSeries.update({ time: timeSec, value: data.equity }); } catch (e) {}
    }
  }
  else if (type === 'system_status') {
    const deadmanTag = document.getElementById('deadman-tag');
    if (deadmanTag) {
      deadmanTag.innerText = `🛡️ Deadman's Switch: ${data.collector_status}`;
    }
    const activeOrdersEl = document.getElementById('val-active-orders');
    if (activeOrdersEl) {
      activeOrdersEl.innerText = `${data.active_orders_count}건 대기 중`;
    }
  }
}

function addOrderTableRow(timestamp, orderId, side, price, qty, reason, badgeClass) {
  const tbody = document.getElementById('order-table-body');
  if (!tbody) return;

  if (tbody.children.length === 1 && tbody.children[0].children.length === 1) {
    tbody.innerHTML = '';
  }

  const timeStr = new Date(timestamp).toLocaleTimeString();
  const row = document.createElement('tr');
  row.innerHTML = `
    <td>${timeStr}</td>
    <td>#${orderId}</td>
    <td><span class="${badgeClass}">${side}</span></td>
    <td>${price > 0 ? `$${price.toFixed(2)}` : '-'}</td>
    <td>${qty > 0 ? qty.toFixed(4) : '-'}</td>
    <td style="color: var(--seed-color-fg-neutral-muted); font-size: 11px;">${reason}</td>
  `;

  tbody.insertBefore(row, tbody.firstChild);
  if (tbody.children.length > 50) {
    tbody.removeChild(tbody.lastChild);
  }
}

document.addEventListener('DOMContentLoaded', () => {
  initCanvases();
  initPnlChart();
  connectWebSocket();
});
