import pandas as pd
from typing import List, Any
from .config import KLINES_COLUMNS

class DataProcessor:
    @staticmethod
    def process_klines(raw_data: List[List[Any]]) -> pd.DataFrame:
        """
        Converts raw Kline data into a structured pandas DataFrame with correct headers and types.
        
        Args:
            raw_data (List[List[Any]]): Raw Kline data lists from Binance.
            
        Returns:
            pd.DataFrame: Cleaned pandas DataFrame.
        """
        if not raw_data:
            return pd.DataFrame(columns=KLINES_COLUMNS)
            
        if len(raw_data[0]) != len(KLINES_COLUMNS):
            raise ValueError(
                f"Expected {len(KLINES_COLUMNS)} columns from Binance Kline data, "
                f"but got {len(raw_data[0])} columns. Raw columns size mismatch."
            )
            
        df = pd.DataFrame(raw_data, columns=KLINES_COLUMNS)
        
        # Define types
        type_mapping = {
            "open_time": "int64",
            "open": "float64",
            "high": "float64",
            "low": "float64",
            "close": "float64",
            "volume": "float64",
            "close_time": "int64",
            "quote_asset_volume": "float64",
            "number_of_trades": "int64",
            "taker_buy_base_asset_volume": "float64",
            "taker_buy_quote_asset_volume": "float64",
            "ignore": "float64"
        }
        
        for col, col_type in type_mapping.items():
            df[col] = df[col].astype(col_type)
            
        # Add helper human-readable UTC timestamp columns
        df['open_time_utc'] = pd.to_datetime(df['open_time'], unit='ms', utc=True)
        df['close_time_utc'] = pd.to_datetime(df['close_time'], unit='ms', utc=True)
        
        # Reorder to keep datetime columns right next to the millisecond timestamps
        new_cols_order = [
            "open_time_utc", "open_time",
            "open", "high", "low", "close", "volume",
            "close_time_utc", "close_time",
            "quote_asset_volume", "number_of_trades",
            "taker_buy_base_asset_volume", "taker_buy_quote_asset_volume",
            "ignore"
        ]
        
        df = df[new_cols_order]
        return df
