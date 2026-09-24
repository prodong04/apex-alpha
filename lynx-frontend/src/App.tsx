import { useEffect, useState } from '@lynx-js/react';
import './App.css';

interface ExchangeData {
  spotPrice: number;
  spotCount: number;
  futPrice: number;
  futCount: number;
  hlPrice: number;
  hlCount: number;
}

interface AccountData {
  equity: number;
  netPnl: number;
  totalFees: number;
  positionSide: string | null;
  positionSize: number;
}

interface DecisionItem {
  id: string;
  time: string;
  action: string;
  price: number;
  qty: number;
  reason: string;
}

export function App() {
  const [exchanges, setExchanges] = useState<ExchangeData>({
    spotPrice: 0,
    spotCount: 0,
    futPrice: 0,
    futCount: 0,
    hlPrice: 0,
    hlCount: 0,
  });

  const [account, setAccount] = useState<AccountData>({
    equity: 10000,
    netPnl: 0,
    totalFees: 0,
    positionSide: null,
    positionSize: 0,
  });

  const [decisions, setDecisions] = useState<DecisionItem[]>([]);
  const [isLiveMode, setIsLiveMode] = useState<boolean>(false);

  useEffect(() => {
    const ws = new WebSocket('ws://127.0.0.1:3000/ws');

    ws.onmessage = (event) => {
      try {
        const payload = JSON.parse(event.data);
        const { type, data } = payload;

        if (type === 'market_tick') {
          setExchanges((prev) => ({
            ...prev,
            futPrice: data.price,
          }));
        } else if (type === 'exchange_trade') {
          if (data.exchange.includes('Spot')) {
            setExchanges((prev) => ({
              ...prev,
              spotPrice: data.price,
              spotCount: prev.spotCount + 1,
            }));
          } else if (data.exchange.includes('Futures')) {
            setExchanges((prev) => ({
              ...prev,
              futPrice: data.price,
              futCount: prev.futCount + 1,
            }));
          } else if (data.exchange.includes('Hyperliquid')) {
            setExchanges((prev) => ({
              ...prev,
              hlPrice: data.price,
              hlCount: prev.hlCount + 1,
            }));
          }
        } else if (type === 'account_update') {
          setAccount({
            equity: data.equity,
            netPnl: data.net_pnl,
            totalFees: data.total_fees,
            positionSide: data.position_side,
            positionSize: data.position_size,
          });
        } else if (type === 'decision_marker') {
          const item: DecisionItem = {
            id: `${data.timestamp}-${Math.random()}`,
            time: new Date(data.timestamp).toLocaleTimeString(),
            action: data.action,
            price: data.price,
            qty: data.qty,
            reason: data.reason,
          };
          setDecisions((prev) => [item, ...prev.slice(0, 30)]);
        } else if (type === 'system_status') {
          setIsLiveMode(data.live_mode);
        }
      } catch (e) {
        console.error('WS Error:', e);
      }
    };

    return () => ws.close();
  }, []);

  const isProfit = account.netPnl >= 0;

  return (
    <view className="container">
      {/* 1. Header */}
      <view className="seed-header">
        <view className="header-left">
          <text className="karrot-badge">🥕 당근 Seed</text>
          <text className="header-title">Apex 3-Exchange Lynx</text>
        </view>
        <view className="header-right">
          <text className={`seed-pill ${isLiveMode ? 'pill-live' : 'pill-paper'}`}>
            {isLiveMode ? '● 실거래 LIVE' : '● 페이퍼 PAPER'}
          </text>
        </view>
      </view>

      {/* 2. 3-Exchange Cards */}
      <view className="metrics-grid">
        <view className="seed-card">
          <text className="card-label">🟡 Binance Spot (현물)</text>
          <text className="card-value">${exchanges.spotPrice.toLocaleString(undefined, { minimumFractionDigits: 2 })}</text>
          <text className="card-sub">수신 틱: {exchanges.spotCount.toLocaleString()}건</text>
        </view>

        <view className="seed-card">
          <text className="card-label">🟢 Binance Futures (선물)</text>
          <text className="card-value">${exchanges.futPrice.toLocaleString(undefined, { minimumFractionDigits: 2 })}</text>
          <text className="card-sub">수신 틱: {exchanges.futCount.toLocaleString()}건</text>
        </view>

        <view className="seed-card">
          <text className="card-label">🔵 Hyperliquid (DEX)</text>
          <text className="card-value">${exchanges.hlPrice.toLocaleString(undefined, { minimumFractionDigits: 2 })}</text>
          <text className="card-sub">수신 틱: {exchanges.hlCount.toLocaleString()}건</text>
        </view>

        <view className="seed-card">
          <text className="card-label">순자산 (Net Equity)</text>
          <text className={`card-value ${isProfit ? 'text-positive' : 'text-negative'}`}>
            ${account.equity.toLocaleString(undefined, { minimumFractionDigits: 2 })}
          </text>
          <text className="card-sub">순손익: {isProfit ? '+' : ''}${account.netPnl.toFixed(2)} (수수료 포함)</text>
        </view>
      </view>

      {/* 3. Decision Log */}
      <view className="log-panel">
        <view className="panel-head">
          <text className="panel-title">🎯 전략 매매 의사결정 타임라인</text>
          <text className="panel-legend">매매 결정 시점 기록</text>
        </view>
        <scroll-view className="log-list" scroll-y>
          {decisions.length === 0 ? (
            <text className="card-sub">전략 시그널 관찰 중 (Passive Mode)...</text>
          ) : (
            decisions.map((d) => (
              <view key={d.id} className="seed-tile">
                <view className="tile-left">
                  <text className={`badge-action ${d.action === 'BUY' ? 'action-buy' : d.action === 'SELL' ? 'action-sell' : 'action-hold'}`}>
                    {d.action}
                  </text>
                  <text className="tile-desc">${d.price.toFixed(2)} - {d.reason}</text>
                </view>
                <text className="tile-time">{d.time}</text>
              </view>
            ))
          )}
        </scroll-view>
      </view>
    </view>
  );
}
