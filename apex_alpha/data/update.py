import sys
import logging
from datetime import datetime, timedelta
from src.downloader import BinanceFuturesDownloader

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler(sys.stdout)
    ]
)

def run_update():
    print("=================== Binance Futures Bulk Update ===================")
    downloader = BinanceFuturesDownloader()
    
    # 마지막 업데이트 시기와 살짝 겹치게 최근 2일치 데이터를 가져와 업데이트
    # 이렇게 하면 누락된 봉이 있더라도 메꿔지며, 기존 데이터와 중복이 발생해도
    # CSV 저장단에서 open_time 기준으로 중복을 알아서 제거하고 정렬하여 저장합니다.
    overlap_days = 2
    start_time = datetime.now() - timedelta(days=overlap_days)
    
    print(f"Performing bulk update for the last {overlap_days} days (from {start_time})...")
    
    # 모든 활성 심볼에 대해 최근 2일치 데이터 다운로드 및 업데이트
    downloader.download_all_symbols_data(
        interval="1m",
        start_datetime=start_time,
        quote_asset="USDT",
        append=True
    )
    
    print("===================================================================")

if __name__ == "__main__":
    run_update()
