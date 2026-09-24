# Apex Alpha - Quantitative Research & Engineering Guidelines

## 1. Execution, Progress & ETA Logging Requirement (Mandatory)
- 모든 데이터 파이프라인, 백테스트, 몬테카를로 시뮬레이션, 배치 처리 스크립트(Rust, Python 등)는 **반드시 주기적인 진행률(%), 경과 시간(Elapsed Time), 예상 잔여 시간(ETA), 처리 속도(ops/sec, bars/sec)**를 콘솔에 표준 포맷으로 로깅해야 합니다.
- 침묵 상태로 실행되는 것을 방지하기 위해 최소 5%~10% 또는 고정 단위(예: 10만 바)마다 진행 상황을 출력합니다.
- 예시 포맷: `[PROGRESS] 45.0% (945,000 / 2,103,840 bars) | Elapsed: 2m 15s | ETA: 2m 45s | Speed: 7,000 bars/sec`

## 2. Institutional Quant Backtest Standards (Two Sigma Quality)
- **Survivorship Bias Zero**: 상장폐지/파산 종목(LUNA, FTT, SRM 등) 전수 포함.
- **Look-Ahead Bias Zero**: Point-In-Time (PIT) 롤링 유니버스 선정, $t+1$ 체결.
- **P-Hacking Protection**:
  - IS (In-Sample Train 70%)
  - Validation (10% 격리 검증)
  - OOS Test (2024-2025 20% Out-of-Sample)
  - **2026 Blind OOT Lock**: 2026년 데이터는 최종 실전 직전까지 100% 미열람 격리 유지.
- **Realistic Friction**: VIP Taker Fee (왕복 10 bps) + Square-root Market Impact Slippage 차감 필수.
