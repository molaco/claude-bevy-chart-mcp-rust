# Bevy Chart

A high-performance real-time candlestick charting application built with Rust and the Bevy game engine. This project started as an experiment to see how fast we could render financial charts using an ECS architecture, and it turned out pretty well.

![Execution Flow](execution_flow.mmd.svg)

## What is this?

This is a desktop charting application designed for cryptocurrency trading data visualization. It connects to Binance's WebSocket API for live updates and stores historical data in DuckDB. The whole thing runs at 100+ FPS even with thousands of candles on screen.

The main idea was to leverage Bevy's ECS (Entity Component System) for efficient rendering and use GPU-based lines (Gizmos) for technical indicators instead of spawning thousands of entities. Turns out this approach works really well for financial charting.

## Features

### Charting Basics
- **Candlestick charts** with real-time updates from Binance WebSocket
- **Volume bars** with toggleable pane (press `V`)
- **Interactive crosshair** that follows your mouse
- **Pan and zoom** - drag to pan, mouse wheel to zoom (zoom coming soon)
- **Multi-pane layout** with synchronized axes (70% price, 30% volume)
- **Lazy loading** - loads 500 candles initially, then fetches more as you scroll

### Technical Indicators
- Simple Moving Averages: SMA-20, SMA-50, SMA-200
- GPU-accelerated rendering using Bevy Gizmos (no entity overhead)
- Toggle individual SMAs with `1`, `2`, `3` or all with `S`
- More indicators coming soon (RSI, MACD, Bollinger Bands)

### Data Management
- DuckDB database for historical OHLCV storage
- Binance API integration for kline downloads
- Real-time WebSocket streaming
- Multi-timeframe aggregation (1m → 5m → 15m → 1h → 4h → 1d)
- Data import from ZIP archives

### Performance Optimizations
We've put a lot of work into making this fast:
- Sliding window algorithm for moving averages (O(n) instead of O(n²))
- Incremental MA calculations when lazy loading
- Entity pooling for candlesticks and volume bars
- Render caching with dirty-checking
- GPU-based line rendering for indicators

## Getting Started

### Prerequisites
- Rust 1.70+ (we're using Edition 2021)
- A working GPU with OpenGL/Vulkan/Metal support
- Linux, macOS, or Windows (primarily tested on Linux)

### Building

```bash
# Build the main chart application
cargo build --release --package charts

# Build the data loader tool
cargo build --release --package data-loader
```

### Loading Data

Before you can chart anything, you need some data. Use the data-loader tool:

```bash
# Download historical klines from Binance
./target/release/data-loader klines \
  --ticker BTCUSDT \
  --interval 15m \
  --start 2024-01-01 \
  --end 2024-01-31

# Or import from ZIP archives
./target/release/data-loader import --path /path/to/archives

# Check what you've got
./target/release/data-loader stats --detailed
```

### Running the Chart

```bash
./target/release/charts \
  --ticker BTCUSDT \
  --timeframe 15m \
  --start 2024-01-01 \
  --end 2024-01-31
```

The app will connect to Binance WebSocket automatically and start receiving live updates.

## Project Structure

This is a Cargo workspace with several crates:

```
charts/              # Main charting application
├── src/
│   ├── main.rs             # App initialization and plugin setup
│   ├── types.rs            # Core data structures (Chart, Candle, etc.)
│   ├── rendering/          # All rendering systems
│   │   ├── candlesticks.rs
│   │   ├── volume.rs
│   │   ├── crosshair.rs
│   │   ├── grid.rs
│   │   └── indicators.rs
│   ├── websocket.rs        # Real-time data streaming
│   ├── realtime.rs         # Live candle updates
│   ├── interaction.rs      # Mouse/keyboard input
│   ├── aggregation/        # Multi-timeframe aggregation
│   └── api.rs              # HTTP API for fetching klines

data-loader/         # CLI tool for importing data
├── src/
│   ├── main.rs
│   ├── db.rs              # Database operations
│   ├── download/          # Binance API integration
│   └── import/            # ZIP archive handling

chat-ui/             # AI chat interface (experimental)
bevy_ui_text_input/  # Reusable text input plugin
chart-mcp/           # Claude MCP integration
```

## Controls

### Keyboard
- `1` / `2` / `3` - Toggle SMA-20 / SMA-50 / SMA-200
- `S` - Toggle all SMAs
- `V` - Toggle volume pane
- `F` - Take screenshot
- Arrow keys - Change timeframe (coming soon)
- `H` - Auto-fit to data (coming soon)

### Mouse
- **Left click + drag** - Pan the chart
- **Scroll wheel** - Zoom in/out (coming soon)
- **Hover** - Crosshair follows cursor

## Architecture

The execution flow diagram above shows how everything fits together. Here's the quick version:

1. **Entry**: Parse CLI args, validate ticker, load initial data
2. **Plugins**: Register WebSocket and Realtime update plugins
3. **Setup**: Initialize DuckDB connection, load 500 candles, setup entity pools
4. **Main Loop** (60-144 FPS):
   - Handle user input (pan, zoom, toggles)
   - Check for lazy load triggers at viewport edges
   - Render pipeline: grid → candles → indicators → volume → crosshair
   - Update UI (FPS counter, timeframe label)
5. **Background Tasks**:
   - WebSocket receives live kline updates
   - Apply throttled updates to avoid overwhelming the renderer
   - Cleanup old candles outside viewport

See [DOCS.md](charts/DOCS.md) and [ARCHITECT.md](charts/ARCHITECT.md) for detailed architecture documentation.

## Tech Stack

- **Bevy 0.17.2** - ECS game engine for rendering and UI
- **DuckDB** - Embedded SQL database for OHLCV data
- **Tokio-Tungstenite** - WebSocket client
- **Reqwest** - HTTP client for Binance API
- **Chrono** - Date/time handling
- **Clap** - CLI argument parsing

## Performance

On a decent gaming laptop (RTX 3060), this thing runs at 144 FPS with 5000+ candles visible. Moving averages are calculated using a sliding window algorithm which is about 100-1000× faster than the naive approach.

We've also implemented incremental MA updates, so when you scroll and trigger a lazy load, we only calculate the new values instead of recalculating everything.

Entity pooling keeps memory allocation low - candlestick and volume entities are recycled instead of constantly spawned and despawned.

## Current Status

This is still v0.1.0 and very much a work in progress. The core charting works well, but there's a lot of polish and features planned:

**Working:**
- Candlestick rendering with real-time updates
- Volume bars with toggle
- SMA indicators (20, 50, 200)
- WebSocket live data
- Lazy loading
- Entity pooling
- Cross-platform support

**In Progress:**
- Mouse wheel zoom
- More indicators (RSI, MACD)
- Keyboard shortcuts for timeframes
- Bollinger Bands

**Planned:**
- Drawing tools (trendlines, Fibonacci retracements)
- Multiple chart types (line, area, Heikin-Ashi)
- Save/load layouts
- Color themes
- Settings panel

See [NEXT.md](NEXT.md) for the full roadmap.

## Database

Data is stored in DuckDB at `~/.local/share/flowsurface/flowsurface.duckdb` by default. You can specify a custom path with `--db-path`.

Schema:
```sql
CREATE TABLE klines (
    ticker VARCHAR,
    interval VARCHAR,
    open_time BIGINT,
    open DOUBLE,
    high DOUBLE,
    low DOUBLE,
    close DOUBLE,
    volume DOUBLE,
    close_time BIGINT,
    PRIMARY KEY (ticker, interval, open_time)
);
```

## Why Bevy?

Initially this was just "what if we used a game engine for charts?" The ECS architecture turned out to be really nice for this use case:

- Systems naturally separate concerns (rendering, interaction, data loading)
- Entity pooling is built into the paradigm
- GPU-accelerated rendering out of the box
- Cross-platform without extra work
- Great hot-reload during development

The main tradeoff is that Bevy is still relatively young and the API changes between versions. But for this kind of interactive, high-performance visualization, it's been a great fit.

## Contributing

This is a personal project at the moment, but feel free to open issues or PRs if you find bugs or have ideas. The code is structured to make it relatively easy to add new indicators - check out `charts/src/rendering/indicators.rs` for examples.

## License

MIT or Apache 2.0, your choice (standard Rust convention).

## Acknowledgments

- Bevy community for the excellent game engine
- Binance for the WebSocket API
- DuckDB team for the fast embedded database
- Everyone who's contributed to the Rust charting/trading ecosystem
