import logging
import sys
import argparse
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

def main():
    parser = argparse.ArgumentParser(description="Binance Futures Bulk 1m Kline Downloader")
    parser.add_argument("--days", type=int, default=None, help="Number of days to download. If not specified, downloads full history.")
    parser.add_argument("--symbols", type=str, default=None, help="Comma-separated symbols to download (e.g., BTCUSDT,ETHUSDT). Defaults to all active symbols.")
    args = parser.parse_args()

    print("=================== Binance Futures Bulk 1m Downloader ===================")
    downloader = BinanceFuturesDownloader()
    
    # 1. Determine period
    start_datetime = None
    if args.days is not None:
        start_datetime = datetime.now() - timedelta(days=args.days)
        print(f"Target Period: Last {args.days} days (from {start_datetime}) to present.")
    else:
        print("Target Period: FULL HISTORY (from the earliest available historical data to present).")
        
    # 2. Get target symbols and download
    if args.symbols:
        target_symbols = [s.strip().upper() for s in args.symbols.split(",")]
        print(f"Target Symbols (User specified): {target_symbols}")
        
        for symbol in target_symbols:
            print(f"\nDownloading {symbol}...")
            downloader.download_symbol_data(
                symbol=symbol,
                interval="1m",
                start_datetime=start_datetime,
                append=True
            )
    else:
        print("Querying all active USDT perpetual symbols from Binance Futures exchangeInfo...")
        downloader.download_all_symbols_data(
            interval="1m",
            start_datetime=start_datetime,
            quote_asset="USDT",
            append=True
        )

    print("==========================================================================")

if __name__ == "__main__":
    main()
