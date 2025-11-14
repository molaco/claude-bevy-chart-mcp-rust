# Data Loader CLI

Command-line tool for downloading and loading market data into DuckDB.

## Features

- **Download klines/candles**: Fetch OHLCV data from Binance
- **Download trades**: Fetch trade data from Binance
- **Import ZIP archives**: Bulk load Binance historical data archives
- **Database management**: Initialize schema, get statistics, vacuum/optimize
- **Export data**: Export trades/klines to CSV

## Installation

```bash
cargo build --release --package data-loader
```

The binary will be at `target/release/data-loader`.

## Usage

### Basic Commands

```bash
# Show help
data-loader --help

# Initialize database
data-loader --db-path /path/to/db.duckdb init

# Show database statistics
data-loader --db-path /path/to/db.duckdb stats

# Show detailed statistics
data-loader stats --detailed

# Vacuum/optimize database
data-loader vacuum
```

### Download Data (TODO)

```bash
# Download klines
data-loader klines \
  --ticker BTCUSDT \
  --interval 15m \
  --start 2024-01-01 \
  --end 2024-01-31

# Download trades
data-loader trades \
  --ticker BTCUSDT \
  --start 2024-01-01 \
  --end 2024-01-31
```

### Import Archives (TODO)

```bash
# Import Binance ZIP archives
data-loader import \
  --path /path/to/binance/archives \
  --batch-size 5000

# Dry run (don't write to database)
data-loader import \
  --path /path/to/archives \
  --dry-run
```

### Export Data (TODO)

```bash
# Export trades to CSV
data-loader export \
  --ticker BTCUSDT \
  --start 2024-01-01 \
  --end 2024-01-31 \
  --output trades.csv \
  --data-type trades

# Export klines to CSV
data-loader export \
  --ticker BTCUSDT \
  --interval 15m \
  --start 2024-01-01 \
  --end 2024-01-31 \
  --output klines.csv \
  --data-type klines
```

## Database Path

Default database path: `/home/molaco/.local/share/flowsurface/flowsurface.duckdb`

Override with `--db-path` flag:
```bash
data-loader --db-path /custom/path.db stats
```

## Logging

```bash
# Enable verbose logging
data-loader --verbose stats

# Or use RUST_LOG environment variable
RUST_LOG=debug data-loader stats
```

## Database Schema

The tool creates/uses these tables:
- `schema_version`: Schema versioning
- `exchanges`: Exchange definitions
- `tickers`: Trading pair information
- `trades`: Individual trades
- `klines`: OHLCV candlestick data

## Development Status

✅ **Implemented**:
- CLI structure with clap
- Database initialization
- Schema creation
- Statistics display
- Vacuum/optimize

🚧 **TODO**:
- Download klines from Binance API
- Download trades from Binance API
- Import ZIP archives (copy from flowsurface-binance)
- Export to CSV
- Detailed per-ticker statistics

## Integration

### With Bevy Chart App

The chart app at `charts/` reads from the same DuckDB database:

```rust
// In charts/src/main.rs
let db_path = "/home/molaco/.local/share/flowsurface/flowsurface.duckdb";
let db = ChartDatabase::new(db_path).expect("Failed to open database");
```

### Scheduled Downloads

Use cron or systemd timers:

```bash
# Crontab example - download daily at 1 AM
0 1 * * * /path/to/data-loader klines --ticker BTCUSDT --interval 15m --start $(date -d yesterday +\%Y-\%m-\%d) --end $(date -d yesterday +\%Y-\%m-\%d)
```

## Next Steps

1. **Copy migration code** from `/home/molaco/Documents/flowsurface-binance/data/src/db/migration/`
   - `archive.rs` - ZIP import logic
   - `helpers.rs` - Ticker ID management, trade ID generation
   
2. **Add Binance API client** (reuse from flowsurface or add `reqwest` + API code)

3. **Implement download commands** with progress bars using `indicatif`

4. **Add CSV export** functionality

## License

Same as parent workspace
