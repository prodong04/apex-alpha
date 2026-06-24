# Binance Futures 1m Candlestick Downloader

A highly modularized, robust Python package to fetch 1-minute candlestick (Kline) data from the Binance Futures API, parse all columns with appropriate data types, and store them securely into CSV files within the `apex_alpha/data/` directory.

## 1. Directory Structure

The package is structured as follows inside the `apex_alpha/data/` workspace:

```
apex-alpha/
├── .gitignore                    # Ignores all *.csv files anywhere in the repo
└── apex_alpha/
    └── data/                     # Primary workspace root directory
        ├── .gitkeep              # Empty file to track data directory in Git
        ├── BTCUSDT_1m.csv        # Downloader output (Git-ignored)
        ├── run.py                # Test orchestrator and demonstrator script
        ├── update.py             # Incremental bulk updater script
        ├── downloader_setup_report.md # This specification report
        └── src/
            ├── __init__.py       # Package initializer exposing key classes
            ├── config.py         # Global constants & Binance column definitions
            ├── binance_client.py # HTTP Client with retry and rate-limiting handles
            ├── data_processor.py # Pandas DataFrame parser & type converter
            └── storage.py        # CSV saving with automatic duplicate-resolution
```

---

## 2. Modularity & Responsibilities

Each file has a single, well-defined responsibility:

### [config.py](file:///Users/daniel/Documents/apex-alpha/apex_alpha/data/src/config.py)
* Defines directories (`BASE_DIR`, `DEFAULT_DATA_DIR`).
* Defines Binance Futures endpoint constants (`BINANCE_FUTURES_BASE_URL`, `/fapi/v1/klines`).
* Defines the 12 fields returned by the Binance API in the exact order:
  `["open_time", "open", "high", "low", "close", "volume", "close_time", "quote_asset_volume", "number_of_trades", "taker_buy_base_asset_volume", "taker_buy_quote_asset_volume", "ignore"]`.

### [binance_client.py](file:///Users/daniel/Documents/apex-alpha/apex_alpha/data/src/binance_client.py)
* Uses `requests.Session` for persistent connections.
* Formulates request queries for Binance Futures API.
* Implements defensive error protection (captures `429/418` rate limits and respects the `Retry-After` header; performs exponential backoff on connection errors).
* Provides `fetch_active_symbols` to list all currently trading USDT perpetual tickers.

### [data_processor.py](file:///Users/daniel/Documents/apex-alpha/apex_alpha/data/src/data_processor.py)
* Receives raw nested lists of candlestick data.
* Assigns columns and casts them into clean, precise pandas types (`int64` and `float64`).
* Preserves **all 12 original columns** without missing any, and adds `open_time_utc` and `close_time_utc` for human-readability.

### [storage.py](file:///Users/daniel/Documents/apex-alpha/apex_alpha/data/src/storage.py)
* Handles checking and creating output directories.
* Implements an **incremental updating mechanism**: if a CSV file already exists, it loads the existing CSV, appends new rows, drops duplicates based on `open_time`, sorts by timestamp, and overwrites the CSV file cleanly.

### [downloader.py](file:///Users/daniel/Documents/apex-alpha/apex_alpha/data/src/downloader.py)
* Coordinates the client, processor, and storage components.
* Provides **backward pagination (`download_historical_range_backward`)**: starts from `end_datetime` (present) and paginates backwards in chunks of 1500 until the target start datetime is reached, or until the API returns an empty list, which signals the beginning of history.
* Provides `download_symbol_data` to request full history (default) or specified timeframes for a single symbol.
* Provides `download_all_symbols_data` as a wrapper that loops through all symbols.

---

## 3. Data Schema & Types

The generated CSV maps exactly to the following pandas structure:

| Column | Type | Description |
| :--- | :--- | :--- |
| **`open_time_utc`** | `datetime64[ns, UTC]` | Human-readable UTC opening time (Helper) |
| **`open_time`** | `int64` | Opening timestamp in milliseconds |
| **`open`** | `float64` | Opening price |
| **`high`** | `float64` | Highest price during the interval |
| **`low`** | `float64` | Lowest price during the interval |
| **`close`** | `float64` | Closing price |
| **`volume`** | `float64` | Total volume in base asset |
| **`close_time_utc`** | `datetime64[ns, UTC]` | Human-readable UTC closing time (Helper) |
| **`close_time`** | `int64` | Closing timestamp in milliseconds |
| **`quote_asset_volume`** | `float64` | Total volume in quote asset |
| **`number_of_trades`** | `int64` | Number of trades |
| **`taker_buy_base_asset_volume`** | `float64` | Taker buy base asset volume |
| **`taker_buy_quote_asset_volume`** | `float64` | Taker buy quote asset volume |
| **`ignore`** | `float64` | System ignore value |

---

## 4. Git Ignore Rules

To ensure raw CSV data files are never committed into git while keeping the project structure tracked, the root `.gitignore` contains:

```gitignore
# Data files (Ensure CSV files are ignored, while allowing code/modules inside the data directory to be tracked)
*.csv
```
