# Hyperliquid Complete Real-Time Market Data Pipeline (Rust / Docker)

초저지연(Ultra Low-Latency) 및 고가용성을 위해 구축된 **Hyperliquid DEX 실시간 체결 틱(Trades with TID) + 20단계 전체 호가 깊이(Full 20-Depth L2 Book) + 자산 핵심 메트릭(Funding / Open Interest / Mark & Oracle / Impact Prices)** 올인원 수집 엔진입니다.

가비지 컬렉터(GC)가 없는 **Rust(`tokio` 비동기 런타임)** 기반으로 작동하여 초당 수만 건의 틱 유입 시에도 **메모리 15~25MB, CPU 0%대**로 24시간 365일 무중단 수집이 가능합니다.

---

## 📂 수집 데이터 3종 스펙 (Data Schema)

저장 디렉토리: `data/live/`

### 1. `[coin]_trades_live.csv` (실시간 개별 체결 틱 전수)
모든 체결 틱이 1건도 누락되지 않고 100% 전수 기록됩니다.

| 칼럼명 | 설명 | 예시 |
| :--- | :--- | :--- |
| `timestamp_ms` | 체결 타임스탬프 (Unix epoch ms) | `1787240008098` |
| `datetime_utc` | UTC 변환 일시 (밀리초 정밀도) | `2026-08-20 15:33:28.098` |
| `coin` | 심볼명 | `BTC` |
| `side` | 체결 방향 (`B`: 매수 / `A`: 매도) | `B` |
| `price` | 체결 가격 | `72623.0` |
| `size` | 체결 수량 (BTC) | `0.45883` |
| `hash` | 온체인 트랜잭션 고유 해시 | `0x454bcee0...` |
| `tid` | **체결 고유 시퀀스 ID (누락 검증용)** | `102938481` |

---

### 2. `[coin]_l2book_live.csv` (20단계 전체 호가 깊이)
매 갱신 시점마다 BBO(최우선 호가/스프레드)와 **매수 20단계 + 매도 20단계의 전체 호가창 깊이**를 전수 기록합니다.

| 칼럼명 | 설명 | 예시 |
| :--- | :--- | :--- |
| `timestamp_ms` | 스냅샷 타임스탬프 (ms) | `1787240015000` |
| `datetime_utc` | UTC 변환 일시 | `2026-08-20 15:33:35.000` |
| `coin` | 심볼명 | `BTC` |
| `best_bid_px` / `best_bid_sz` | 최우선 매수 호가 및 잔량 | `72622.0` / `15.4210` |
| `best_ask_px` / `best_ask_sz` | 최우선 매도 호가 및 잔량 | `72623.0` / `3.8205` |
| `spread` | 호가 스프레드 ($) | `1.00` |
| **`bids`** | **매수 20단계 전체 배열 `[{"px", "sz", "n"}, ...]`** | `"[{\"px\":\"72622.0\",\"sz\":\"15.4\",\"n\":3},...]"` |
| **`asks`** | **매도 20단계 전체 배열 `[{"px", "sz", "n"}, ...]`** | `"[{\"px\":\"72623.0\",\"sz\":\"3.8\",\"n\":1},...]"` |

---

### 3. `[coin]_metrics_live.csv` (실시간 자산 메트릭 & 펀딩/OI)
시장의 자금 유입과 슬리피지, 롱/숏 포지션 변화를 실시간 기록합니다.

| 칼럼명 | 설명 | 예시 |
| :--- | :--- | :--- |
| `timestamp_ms` / `datetime_utc` | 수신 타임스탬프 | `1787240015123` / `2026-08-20 15:33:35.123` |
| `funding_rate` | **실시간 펀딩비율** (시간당) | `0.0000125` |
| `open_interest` | **미결제약정(OI, 오픈 포지션 총량)** | `1542.58` BTC |
| `oracle_px` / `mark_px` | 오라클 현물 중앙값 / 마크 프라이스 | `72620.5` / `72622.0` |
| `mid_px` / `premium` | 오더북 중간 가격 / 선물 프리미엄 괴리율 | `72622.5` / `0.00002` |
| `impact_bid_px` / `impact_ask_px` | **$100k 시장가 매수/매도 시 예상 슬리피지 가격** | `72621.0` / `72624.5` |
| `day_ntl_vlm` | 24시간 누적 거래대금 ($) | `854219000.0` |

---

## 🚀 빠른 시작 가이드

```bash
# 1. 최신 코드 가져오기
git pull

# 2. 기존 컨테이너 교체 실행
sudo docker stop hl-collector && sudo docker rm hl-collector
sudo docker build -t hl-collector .
sudo docker run -d --name hl-collector -v $(pwd)/data:/app/data hl-collector

# 3. 실시간 로그 확인
sudo docker logs -f hl-collector
```
