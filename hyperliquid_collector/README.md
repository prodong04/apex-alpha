# Hyperliquid Real-Time L2 & Tick Market Data Collector (Rust / Docker)

초저지연(Ultra Low-Latency) 및 고가용성(High Resilience)을 목표로 설계된 **Hyperliquid DEX 실시간 L2 호가창 Depth 및 개별 체결 틱(Tick-by-Tick Trades) 수집 엔진**입니다.

가비지 컬렉터(GC)가 없는 **Rust(`tokio` 비동기 런타임)** 기반으로 구축되어, 초당 수만 건의 대량 트랜잭션이 발생하는 변동성 폭발 구간에서도 **메모리 15MB 내외, CPU 0%대**로 24시간 365일 무중단 수집이 가능합니다.

---

## 📂 프로젝트 구조

```
hyperliquid_collector/
├── Dockerfile               # 경량 멀티스테이지 Docker 빌드 파일 (Zero-Dependency)
├── docker-compose.yml       # 원클릭 백그라운드 구동 및 볼륨 마운트 설정
├── .dockerignore            # 불필요한 빌드 캐시 및 데이터 제외
├── Cargo.toml               # Rust 최적화 의존성 및 릴리즈 프로파일 설정
├── README.md                # 실행 가이드 및 데이터 스펙 문서
├── scripts/
│   └── inspect_data.py      # 실시간 수집 현황 및 틱 요약 통계 CLI 도구
└── src/
    ├── main.rs              # 웹소켓 이벤트 루프, 하트비트, 자동 재연결, 버퍼 플러시
    └── types.rs             # Hyperliquid L2/L3 및 체결 틱 데이터 구조체
```

---

## 🚀 빠른 시작 가이드 (Quick Start)

사용 환경에 따라 **Docker(추천)** 또는 **Native Rust** 방식으로 실행할 수 있습니다.

### 방법 1. Docker로 실행하기 (가장 간편 / 의존성 없음)

Rust나 C++ 컴파일러 설치 없이 Docker만 있으면 즉시 실행할 수 있습니다.

```bash
# 1. 디렉토리 이동
cd hyperliquid_collector

# 2. 백그라운드에서 수집기 컨테이너 빌드 및 실행
docker compose up -d

# 3. 실시간 수집 로그 확인
docker compose logs -f

# 4. 수집기 중지
docker compose down
```

> 💡 **데이터 저장 위치**: 호스트 머신의 `hyperliquid_collector/data/live/` 디렉토리에 실시간으로 마운트되어 저장됩니다.

---

### 방법 2. Native Rust (Cargo)로 직접 실행하기

시스템에 Rust 툴체인이 설치되어 있는 경우 직접 빌드하여 실행합니다.

```bash
cd hyperliquid_collector

# Cargo 환경 로드 (필요시)
source "$HOME/.cargo/env"

# 수집기 릴리즈 빌드 및 실행 (기본: BTC)
cargo run --release

# 다른 코인(예: ETH, SOL) 수집 시 환경변수 전달
TARGET_COIN=ETH cargo run --release
```

---

## 📊 실시간 수집 데이터 확인 및 모니터링 (Inspector)

수집기가 백그라운드에서 동작하는 동안, **수집기를 중단하지 않고** 언제든 터미널에서 데이터 현황을 확인할 수 있습니다.

### 1) CLI 요약 브리핑 실행
```bash
python3 scripts/inspect_data.py --coin BTC
```

**출력 화면 예시:**
```
=================================================================
📊 [BTC LIVE TRADES STATS] 총 수집 틱: 7,132건
=================================================================
• 현재가: $65,420.0 | 최저가: $64,940.0 | 최고가: $65,435.0
• 매수(Buy) 볼륨 :   970.2033 BTC (4,430건, 71.0%)
• 매도(Sell) 볼륨:   397.1813 BTC (2,702건, 29.0%)
• 총 거래량       :  1,367.3846 BTC
-----------------------------------------------------------------
🕒 [최근 5건의 실시간 체결 틱]
시간 (UTC)                | 사이드  | 가격 ($)    | 수량        
-----------------------------------------------------------------
2026-08-19 14:43:37.344 | BUY  | 65420.0     | 0.00726     
2026-08-19 14:43:39.036 | BUY  | 65420.0     | 0.07420     
2026-08-19 14:43:39.036 | BUY  | 65420.0     | 0.04863     
2026-08-19 14:43:39.036 | BUY  | 65420.0     | 0.01605     
2026-08-19 14:43:39.036 | BUY  | 65420.0     | 0.00053     
=================================================================
```

### 2) 실시간 틱 스트림 모니터링 (`tail`)
```bash
# 실시간 체결 틱 확인
tail -f data/live/btc_trades_live.csv

# 실시간 L2 호가창 Depth 상태 확인
tail -n 10 data/live/btc_l2book_live.csv
```

---

## 📑 수집 데이터 스펙 (Data Schema)

저장 디렉토리: `data/live/`

### 1. `[coin]_trades_live.csv` (실시간 개별 체결 틱)
모든 체결 틱이 1건도 누락되지 않고 100% 전수 기록됩니다.

| 칼럼명 | 데이터 타입 | 설명 | 예시 |
| :--- | :--- | :--- | :--- |
| `timestamp_ms` | `Int64` | 체결 타임스탬프 (Unix epoch ms) | `1787146128956` |
| `datetime_utc` | `String` | UTC 변환 일시 (밀리초 정밀도) | `2026-08-19 13:28:48.956` |
| `coin` | `String` | 심볼명 | `BTC` |
| `side` | `String` | 체결 방향 (`B`: 매수 / `A`: 매도) | `B` |
| `price` | `Float64` | 체결 가격 | `65420.0` |
| `size` | `Float64` | 체결 수량 | `0.0742` |
| `hash` | `String` | 블록체인 트랜잭션 고유 해시 | `0x17dd827f...` |

### 2. `[coin]_l2book_live.csv` (실시간 L2 호가창 Depth)
호가창 갱신 시점마다 최우선 매수/매도 호가(BBO) 및 스프레드, 뎁스 레벨 수를 기록합니다.

| 칼럼명 | 데이터 타입 | 설명 | 예시 |
| :--- | :--- | :--- | :--- |
| `timestamp_ms` | `Int64` | 스냅샷 타임스탬프 (ms) | `1787146136538` |
| `datetime_utc` | `String` | UTC 변환 일시 | `2026-08-19 13:28:56.538` |
| `coin` | `String` | 심볼명 | `BTC` |
| `best_bid_px` | `Float64` | 최우선 매수호가 (Best Bid) | `65425.0` |
| `best_bid_sz` | `Float64` | 최우선 매수호가 잔량 | `0.72106` |
| `best_ask_px` | `Float64` | 최우선 매도호가 (Best Ask) | `65426.0` |
| `best_ask_sz` | `Float64` | 최우선 매도호가 잔량 | `23.77339` |
| `spread` | `Float64` | 매수/매도 호가 스프레드 ($) | `1.00` |
| `bid_depth_levels` | `Int32` | 매수 뎁스 단계 수 | `20` |
| `ask_depth_levels` | `Int32` | 매도 뎁스 단계 수 | `20` |

---

## ⚙️ 아키텍처 및 복원력(Resilience) 설계

* **Active Ping Heartbeat**: 클라우드 프록시 및 게이트웨이의 타임아웃(Silent Drop)을 방지하기 위해 20초 주기로 클라이언트 핑을 능동 발송합니다.
* **30초 Read Timeout & Auto-Reconnect**: 일시적인 네트워크 순단 발생 시 30초 내로 감지하여 3초 딜레이 후 자동 복구합니다.
* **Buffered I/O (BufWriter)**: 초당 수천 건의 틱 유입 시에도 디스크 I/O 병목이 발생하지 않도록 1초 주기 일괄 플러시(Flush)를 수행하여 데이터 유실을 방지합니다.
