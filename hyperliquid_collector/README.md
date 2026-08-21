# 🚀 Unified High-Frequency Crypto Market Data Engine (Rust)

초저지연(Ultra Low-Latency) 및 고가용성을 위해 구축된 **바이낸스 현물(Spot) + 바이낸스 선물(Futures) + 하이퍼리퀴드(Hyperliquid DEX)** 통합 실시간 오더북 / 체결 틱 / 펀딩비 / 청산 / 센티먼트 수집 파이프라인입니다.

가비지 컬렉터(GC)가 없는 **Rust(`tokio` 멀티스레드 비동기 런타임)** 기반으로 작동하여 초당 수만 건의 틱 유입 시에도 **메모리 20~30MB, CPU 0%대**로 24시간 365일 무중단 수집됩니다.

---

## 📊 수집 데이터 구조 (Directory & File Schema)

데이터 저장 루트: `data/live/`

```
data/live/
├── binance_spot/
│   ├── btc_depth20_live.csv       # [현물] 100ms 간격 20단계 호가 깊이 (Orderbook)
│   ├── btc_bbo_live.csv           # [현물] 실시간 0ms BBO 최우선 호가/스프레드
│   └── btc_aggtrades_live.csv     # [현물] 실시간 체결 틱 전수 (AggTrades)
│
├── binance_futures/
│   ├── btc_depth20_live.csv       # [선물] 100ms 간격 20단계 호가 깊이 (Orderbook)
│   ├── btc_bbo_live.csv           # [선물] 실시간 0ms BBO 최우선 호가/스프레드
│   ├── btc_aggtrades_live.csv     # [선물] 실시간 체결 틱 전수 (AggTrades)
│   ├── btc_mark_funding_live.csv  # [선물] 1초 간격 마크 프라이스 & 실시간 펀딩비
│   ├── btc_liquidations_live.csv  # [선물] 실시간 강제청산(Liquidations) 체결 틱
│   └── btc_metrics_live.csv       # [선물] 10초 주기 미결제약정(OI) & 롱숏비/고래포지션
│
└── hyperliquid/
    ├── btc_depth20_live.csv       # [DEX] ~5초 주기 20단계 호가 깊이 (L2 Book)
    ├── btc_trades_live.csv        # [DEX] 실시간 체결 틱 (TID 포함)
    └── btc_metrics_live.csv       # [DEX] 1초 주기 펀딩비 / 미결제약정 / 오라클 / 슬리피지
```

---

## 🔍 수집 항목 상세 스펙

### 1. 바이낸스 현물 (`binance_spot`)
- **`btc_depth20_live.csv`**: `last_update_id`, `local_ts`, `local_datetime_utc`, `coin`, `best_bid_px`, `best_bid_sz`, `best_ask_px`, `best_ask_sz`, `spread`, `bids (20-depth JSON)`, `asks (20-depth JSON)`
- **`btc_bbo_live.csv`**: `update_id`, `local_ts`, `local_datetime_utc`, `coin`, `bid_px`, `bid_sz`, `ask_px`, `ask_sz`, `spread`
- **`btc_aggtrades_live.csv`**: `agg_trade_id`, `trade_time_ms`, `event_time_ms`, `local_ts`, `trade_datetime_utc`, `coin`, `price`, `qty`, `first_trade_id`, `last_trade_id`, `is_buyer_maker`

### 2. 바이낸스 선물 (`binance_futures`)
- **`btc_depth20_live.csv`**: 100ms L2 20단계 호가창 및 스프레드
- **`btc_bbo_live.csv`**: 실시간 BBO 최우선 호가 잔량 변화
- **`btc_aggtrades_live.csv`**: 실시간 테이커 체결 틱
- **`btc_mark_funding_live.csv`**: `mark_px`, `index_px`, `est_settle_px`, `funding_rate`, `next_funding_time_ms`
- **`btc_liquidations_live.csv`**: `side (BUY/SELL 청산)`, `orig_qty`, `price`, `avg_price`, `accum_filled_qty`
- **`btc_metrics_live.csv`**: `open_interest`, `top_trader_long_short_pos_ratio` (상위 트레이더 포지션 롱숏비), `top_trader_long_short_acc_ratio` (상위 계정 롱숏비), `global_long_short_acc_ratio` (전체 계정 롱숏비), `taker_buy_sell_vol_ratio` (테이커 매수/매도 볼륨 비율)

### 3. 하이퍼리퀴드 DEX (`hyperliquid`)
- **`btc_depth20_live.csv`**: 20-depth Orderbook
- **`btc_trades_live.csv`**: 실시간 체결 틱 (TID 시퀀스 포함)
- **`btc_metrics_live.csv`**: 1초 주기 펀딩비, 미결제약정(OI), 오라클 가격, $100k 시장가 충격 슬리피지 가격(`impact_bid_px`, `impact_ask_px`)

---

## 🛠️ 실행 및 배포 가이드

### 방법 1: 로컬 / 서버에서 직접 실행 (Rust Cargo)
```bash
# 릴리스 빌드
cargo build --release

# 실행 (기본값: BTC, 모든 거래소 활성화)
./target/release/market-collector

# 환경 변수로 특정 거래소만 활성화하거나 코인 변경 가능
TARGET_COIN=ETH ENABLE_HYPERLIQUID=false ./target/release/market-collector
```

### 방법 2: Docker / Docker Compose로 백그라운드 배포
```bash
# Docker Compose로 빌드 및 백그라운드 실행
docker compose up -d --build

# 실시간 수집 로그 확인
docker compose logs -f

# 컨테이너 상태 확인
docker ps
```

---

## 📈 수집 데이터 실시간 통계 검사기 (Inspector)

수집 중인 데이터의 각 거래소별 총 레코드 수, 파일 크기, 최신 체결가, 펀딩비, 롱숏비 등을 한눈에 검사할 수 있는 Python 스크립트가 내장되어 있습니다:

```bash
python3 scripts/inspect_data.py
```
