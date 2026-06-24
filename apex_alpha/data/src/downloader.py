import time
import logging
from typing import Optional, Dict, Any, List
import pandas as pd
from datetime import datetime

from .binance_client import BinanceFuturesClient
from .data_processor import DataProcessor
from .storage import CSVStorage
from .config import DEFAULT_DATA_DIR, BINANCE_FUTURES_BASE_URL

logger = logging.getLogger(__name__)

class BinanceFuturesDownloader:
    def __init__(self, data_dir: str = DEFAULT_DATA_DIR, base_url: str = BINANCE_FUTURES_BASE_URL):
        self.client = BinanceFuturesClient(base_url=base_url)
        self.storage = CSVStorage(data_dir=data_dir)
        
    def download_klines(
        self,
        symbol: str,
        interval: str,
        start_time_ms: Optional[int] = None,
        end_time_ms: Optional[int] = None,
        limit: int = 500,
        append: bool = True
    ) -> pd.DataFrame:
        """
        Downloads, processes, and saves klines for a given symbol and interval.
        
        Args:
            symbol (str): Asset symbol (e.g. 'BTCUSDT').
            interval (str): Timeframe interval (e.g. '1m').
            start_time_ms (int, optional): Start timestamp in milliseconds.
            end_time_ms (int, optional): End timestamp in milliseconds.
            limit (int): Number of candles to fetch (default 500).
            append (bool): Whether to append to existing files.
            
        Returns:
            pd.DataFrame: The processed DataFrame that was downloaded.
        """
        logger.info(f"Starting download process for {symbol} ({interval})...")
        
        # 1. Fetch raw data
        raw_data = self.client.fetch_klines(
            symbol=symbol,
            interval=interval,
            start_time=start_time_ms,
            end_time=end_time_ms,
            limit=limit
        )
        
        # 2. Process to DataFrame
        df = DataProcessor.process_klines(raw_data)
        
        # 3. Save to CSV
        self.storage.save_data(df, symbol, interval, append=append)
        
        return df

    def download_historical_range_backward(
        self,
        symbol: str,
        interval: str,
        start_datetime: datetime,
        end_datetime: datetime,
        append: bool = True
    ) -> pd.DataFrame:
        """
        Downloads a range of data by paginating backwards from end_datetime to start_datetime.
        
        Args:
            symbol (str): Asset symbol.
            interval (str): Timeframe.
            start_datetime (datetime): Start datetime object.
            end_datetime (datetime): End datetime object.
            append (bool): Append to file.
        """
        start_ms = int(start_datetime.timestamp() * 1000)
        end_ms = int(end_datetime.timestamp() * 1000)
        
        current_end = end_ms
        all_dfs = []
        
        logger.info(f"Downloading historical range backward from {end_datetime} to {start_datetime} for {symbol}...")
        
        chunk_count = 0
        while current_end > start_ms:
            # Fetch up to 1500 klines preceding current_end
            raw_data = self.client.fetch_klines(
                symbol=symbol,
                interval=interval,
                end_time=current_end,
                limit=1500
            )
            
            if not raw_data:
                logger.info("No more data returned from Binance (reached the beginning of history).")
                break
                
            df = DataProcessor.process_klines(raw_data)
            if df.empty:
                logger.info("Processed DataFrame is empty. Stopping.")
                break
                
            all_dfs.append(df)
            chunk_count += 1
            
            min_open_time = df['open_time'].min()
            logger.info(f"[{chunk_count}] Fetched {len(df)} rows. Min open time: {pd.to_datetime(min_open_time, unit='ms', utc=True)}")
            
            # Next query end time should be 1 millisecond before the min open time of current chunk
            next_end = min_open_time - 1
            if next_end >= current_end:
                # Prevent infinite loop
                break
            current_end = next_end
            
            # Sleep slightly to respect rate limits
            time.sleep(0.1)
            
        if not all_dfs:
            return pd.DataFrame()
            
        combined = pd.concat(all_dfs, ignore_index=True)
        # Drop duplicates and sort chronologically
        combined.drop_duplicates(subset=["open_time"], keep="last", inplace=True)
        combined.sort_values(by="open_time", ascending=True, inplace=True)
        
        self.storage.save_data(combined, symbol, interval, append=append)
        return combined

    def download_symbol_data(
        self,
        symbol: str,
        interval: str,
        start_datetime: Optional[datetime] = None,
        end_datetime: Optional[datetime] = None,
        append: bool = True
    ) -> pd.DataFrame:
        """
        Downloads data for a single symbol. If start_datetime is not provided,
        it starts from the earliest available historical data (timestamp 0).
        If end_datetime is not provided, it defaults to the current time.
        """
        if start_datetime is None:
            # Epoch start (1970-01-01 in local timezone, corresponding to timestamp 0)
            start_datetime = datetime.fromtimestamp(0)
            logger.info(f"No start_datetime provided for {symbol}. Fetching from the earliest available historical data.")
            
        if end_datetime is None:
            end_datetime = datetime.now()
            
        return self.download_historical_range_backward(
            symbol=symbol,
            interval=interval,
            start_datetime=start_datetime,
            end_datetime=end_datetime,
            append=append
        )


    def download_all_symbols_data(
        self,
        interval: str,
        start_datetime: Optional[datetime] = None,
        end_datetime: Optional[datetime] = None,
        quote_asset: str = "USDT",
        append: bool = True
    ) -> Dict[str, pd.DataFrame]:
        """
        Wrapper that fetches all active trading symbols from Binance Futures and
        downloads the specified period (or full history) for each one.
        """
        symbols = self.client.fetch_active_symbols(quote_asset=quote_asset)
        results = {}
        
        logger.info(f"Starting bulk download for {len(symbols)} symbols...")
        for i, symbol in enumerate(symbols, 1):
            try:
                logger.info(f"[{i}/{len(symbols)}] Processing {symbol}...")
                df = self.download_symbol_data(
                    symbol=symbol,
                    interval=interval,
                    start_datetime=start_datetime,
                    end_datetime=end_datetime,
                    append=append
                )
                results[symbol] = df
                # Sleep a little between symbols to avoid hitting rate limits
                time.sleep(0.5)
            except Exception as e:
                logger.error(f"Failed to download data for {symbol}: {e}")
                
        return results

