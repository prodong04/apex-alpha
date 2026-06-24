import os
import pandas as pd
import logging

logger = logging.getLogger(__name__)

class CSVStorage:
    def __init__(self, data_dir: str):
        self.data_dir = data_dir
        os.makedirs(self.data_dir, exist_ok=True)
        
    def get_file_path(self, symbol: str, interval: str) -> str:
        """Returns the file path for a symbol and interval."""
        filename = f"{symbol.upper()}_{interval}.csv"
        return os.path.join(self.data_dir, filename)
        
    def save_data(self, df: pd.DataFrame, symbol: str, interval: str, append: bool = True) -> str:
        """
        Saves a DataFrame to CSV. If append is True, it merges with existing data
        and resolves duplicates based on open_time.
        
        Args:
            df (pd.DataFrame): The DataFrame to save.
            symbol (str): The symbol of the asset.
            interval (str): The timeframe interval.
            append (bool): If True, appends to existing file and removes duplicates.
            
        Returns:
            str: Path to the saved CSV file.
        """
        file_path = self.get_file_path(symbol, interval)
        
        if df.empty:
            logger.warning(f"DataFrame is empty. Nothing to save for {symbol} ({interval}).")
            return file_path
            
        if append and os.path.exists(file_path):
            try:
                existing_df = pd.read_csv(file_path)
                logger.info(f"Found existing file with {len(existing_df)} rows. Merging...")
                
                # Merge and drop duplicates
                combined_df = pd.concat([existing_df, df], ignore_index=True)
                # Drop duplicates by 'open_time'
                combined_df.drop_duplicates(subset=["open_time"], keep="last", inplace=True)
                # Sort by open_time
                combined_df.sort_values(by="open_time", ascending=True, inplace=True)
                df_to_save = combined_df
            except Exception as e:
                logger.error(f"Failed to merge with existing CSV at {file_path}: {e}. Overwriting instead.")
                df_to_save = df
        else:
            df_to_save = df.sort_values(by="open_time", ascending=True)
            
        df_to_save.to_csv(file_path, index=False)
        logger.info(f"Successfully saved {len(df_to_save)} rows to {file_path}")
        return file_path
