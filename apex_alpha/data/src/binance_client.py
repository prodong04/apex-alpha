import time
import requests
import logging
from typing import List, Any, Optional

logger = logging.getLogger(__name__)

class BinanceFuturesClient:
    def __init__(self, base_url: str = "https://fapi.binance.com"):
        self.base_url = base_url.rstrip('/')
        self.session = requests.Session()
        
    def fetch_klines(
        self,
        symbol: str,
        interval: str,
        start_time: Optional[int] = None,
        end_time: Optional[int] = None,
        limit: int = 500
    ) -> List[List[Any]]:
        """
        Fetches klines (candlestick data) from Binance Futures API.
        
        Args:
            symbol (str): Trading pair symbol (e.g., 'BTCUSDT').
            interval (str): Candlestick interval (e.g., '1m', '5m', '1h', '1d').
            start_time (int, optional): Timestamp in milliseconds.
            end_time (int, optional): Timestamp in milliseconds.
            limit (int): Number of candles to fetch (max 1500).
            
        Returns:
            List[List[Any]]: List of candlestick data lists.
        """
        url = f"{self.base_url}/fapi/v1/klines"
        params = {
            "symbol": symbol.upper(),
            "interval": interval,
            "limit": min(limit, 1500)
        }
        
        if start_time is not None:
            params["startTime"] = start_time
        if end_time is not None:
            params["endTime"] = end_time
            
        max_retries = 3
        backoff_factor = 2.0
        
        for attempt in range(max_retries):
            try:
                logger.info(f"Fetching {interval} klines for {symbol} (params: {params})...")
                response = self.session.get(url, params=params, timeout=10)
                
                # Check for rate limit or IP ban responses (429, 418)
                if response.status_code in (429, 418):
                    retry_after = int(response.headers.get("Retry-After", 10))
                    logger.warning(f"Rate limited (status {response.status_code}). Sleeping for {retry_after}s.")
                    time.sleep(retry_after)
                    continue
                    
                response.raise_for_status()
                return response.json()
            except requests.RequestException as e:
                logger.warning(f"Attempt {attempt + 1} failed: {e}")
                if attempt == max_retries - 1:
                    raise
                time.sleep(backoff_factor ** attempt)
        
        raise RuntimeError("Failed to fetch klines after maximum retries.")

    def fetch_active_symbols(self, quote_asset: str = "USDT") -> List[str]:
        """
        Fetches all active trading symbols from Binance Futures.
        Filters by status == 'TRADING', quoteAsset == 'USDT', and contractType == 'PERPETUAL'.
        """
        url = f"{self.base_url}/fapi/v1/exchangeInfo"
        try:
            logger.info("Fetching exchange info to get active symbols...")
            response = self.session.get(url, timeout=10)
            response.raise_for_status()
            data = response.json()
            
            active_symbols = []
            for item in data.get("symbols", []):
                if (item.get("status") == "TRADING" and 
                    item.get("quoteAsset") == quote_asset.upper() and
                    item.get("contractType") == "PERPETUAL"):
                    active_symbols.append(item.get("symbol"))
            
            logger.info(f"Found {len(active_symbols)} active {quote_asset} trading pairs.")
            return active_symbols
        except Exception as e:
            logger.error(f"Failed to fetch active symbols: {e}")
            raise

