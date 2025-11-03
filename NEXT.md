# Next Steps for Bevy Chart Application

This document outlines potential next steps for the charting application, organized by priority and impact.

---

## 🎯 High Priority / High Impact

### 1. **Technical Indicators** (1-3 days each)
**Why:** Core feature of any professional charting app

**Options:**
- Moving Averages (SMA, EMA) - overlayed on price
- RSI (Relative Strength Index) - new pane
- MACD (Moving Average Convergence Divergence) - new pane
- Bollinger Bands - overlayed on price
- Volume Profile

**Complexity:** Medium

**Implementation needs:**
- Indicator calculation logic
- New pane types for RSI/MACD
- Overlay rendering on price pane
- Toggle indicators on/off (keyboard shortcuts)

### 2. **Chart Type Alternatives** (2-3 days)
**Why:** Different visualization preferences

**Options:**
- Line chart (just close prices)
- Area chart (filled line)
- Heikin-Ashi candles (smoothed price action)
- Hollow candles (TradingView style)

**Complexity:** Medium

**Implementation needs:**
- Modify rendering logic
- Add toggle UI (press 'C' to cycle?)
- Keep candlestick data model unchanged

### 3. **Drawing Tools** (3-5 days)
**Why:** Critical for technical analysis

**Options:**
- Trend lines (click + drag)
- Horizontal support/resistance lines
- Fibonacci retracements
- Rectangles/boxes
- Text annotations

**Complexity:** High

**Implementation needs:**
- Mouse event handling for drawing mode
- Persistent drawing entities
- Edit/delete drawings
- Save drawings to database

---

## 🚀 Medium Priority / Good UX

### 4. **Mouse Wheel Zoom** (2-3 hours)
**Why:** Standard interaction pattern

**Features:**
- Scroll wheel = zoom in/out at cursor position
- Much better UX than current drag zoom
- Zoom towards cursor (not center)

**Complexity:** Low

**Implementation needs:**
- Listen to mouse wheel events
- Calculate new visible_candle_count
- Zoom toward cursor position (not center)

### 5. **Keyboard Shortcuts** (1 day)
**Why:** Power user efficiency

**Suggested shortcuts:**
- `+`/`-` = zoom in/out
- `H` = hide/show grid
- `C` = cycle chart types
- `R` = reset zoom to default
- `S` = screenshot/export
- `1-9` = load saved layouts
- `?` = show help overlay

**Complexity:** Low

**Implementation needs:**
- Map keys to functions
- Help overlay UI (optional)

### 6. **Color Themes** (1-2 days)
**Why:** Personalization + accessibility

**Themes:**
- Dark mode (current)
- Light mode
- Solarized
- Monokai
- Custom colors

**Complexity:** Low-Medium

**Implementation needs:**
- Theme resource with color palettes
- Apply to all elements (grid, candles, crosshair, etc.)
- Save preference to config file
- Press `T` to toggle themes

### 7. **Auto-Fit / Reset View** (2-3 hours)
**Why:** Quick navigation

**Features:**
- Double-click chart → fit all data to view
- Press `F` → fit visible candles to height
- Press `Escape` → reset to default view

**Complexity:** Low

**Implementation needs:**
- Calculate min/max for visible range
- Update visible_price_min/max
- Animate transition (optional)

---

## 💎 Polish / Professional Features

### 8. **Save/Load Layouts** (2-3 days)
**Why:** User workflow persistence

**What to save:**
- Pane heights and visibility
- Visible candle range
- Indicators enabled
- Theme preference
- Drawing tools

**Complexity:** Medium

**Implementation needs:**
- Serialize Chart state to JSON/TOML
- Store in `~/.config/chart_app/layout.toml`
- Load on startup
- Auto-save on exit

### 9. **Multiple Timeframes** (1-2 days)
**Why:** Standard feature

**Shortcuts:**
- Press `1` = 1 minute
- Press `5` = 5 minutes
- Press `H` = 1 hour
- Press `D` = 1 day
- Press `W` = 1 week

**Complexity:** Medium

**Implementation needs:**
- UI to switch timeframe
- Query database with new timeframe
- Reload chart data
- Update interval_ms calculations

### 10. **Real-Time Updates** (3-5 days)
**Why:** Live trading charts

**Features:**
- WebSocket connection to data source
- Update last candle in real-time
- Append new candles as they form
- Show "streaming" indicator

**Complexity:** High

**Implementation needs:**
- WebSocket client
- Thread for background updates
- Thread-safe data updates
- Handle reconnection
- Data source API integration

---

## 🔧 Technical / Infrastructure

### 11. **Performance Profiling Suite** (2-3 days)
**Why:** Validate 144fps+ claims

**Features:**
- FPS counter overlay
- Frame time graph
- System stats (CPU, GPU, RAM)
- Benchmark different chart sizes
- Identify bottlenecks

**Complexity:** Medium

**Implementation needs:**
- Bevy diagnostic plugin
- Custom performance overlay
- Export metrics to CSV
- Profiling tools integration

### 12. **Unit Tests** (1-2 weeks)
**Why:** Code quality and confidence

**Test coverage:**
- ChartSpace calculations (to_world, from_world)
- Pane layout logic
- Indicator calculations
- Data loading
- Toggle functionality

**Complexity:** Medium

**Implementation needs:**
- Write test cases
- Mock database for tests
- CI/CD integration
- Test fixtures for candle data

### 13. **Web Deployment (WASM)** (3-5 days)
**Why:** Wider distribution

**Features:**
- Compile to WebAssembly
- Run in browser
- No database (use IndexedDB or fetch from API)
- Host on GitHub Pages

**Complexity:** High

**Implementation needs:**
- WASM-compatible dependencies
- Browser API integration
- Performance considerations
- Different data source (REST API or IndexedDB)
- Build pipeline for WASM

---

## 🎨 UI/UX Improvements

### 14. **Settings Panel** (2-3 days)
**Why:** User customization

**Settings options:**
- Toggle grid/axes/crosshair
- Adjust colors and themes
- Change timeframe
- Enable/disable indicators
- Candle width
- Crosshair dash pattern

**Complexity:** Medium

**Implementation needs:**
- UI overlay with egui or bevy_ui
- Settings resource
- Apply changes in real-time
- Press `S` to open settings panel

### 15. **Price Scale Options** (1-2 days)
**Why:** Different analysis needs

**Scale types:**
- Linear scale (current)
- Logarithmic scale (for long-term trends)
- Percentage scale (relative change)

**Complexity:** Medium

**Implementation needs:**
- Logarithmic transformations in ChartSpace
- Update to_world/from_world calculations
- Toggle with keyboard shortcut
- Adjust Y-axis labels accordingly

---

## 🏆 Advanced / Long-Term

### 16. **Multiple Chart Windows** (1 week)
**Why:** Compare different assets/timeframes

**Features:**
- Multiple Chart resources
- Multiple window management
- Sync crosshair across charts
- Separate camera per chart
- Link zoom/pan (optional)

**Complexity:** High

**Implementation needs:**
- Bevy multi-window management
- Per-window resources
- Event routing per window
- Performance optimization for multiple charts

### 17. **Order Entry / Trading** (2-3 weeks)
**Why:** Full trading application

**Features:**
- Place market/limit orders
- Show open positions on chart
- P&L tracking
- Risk management tools
- Connect to broker API

**Complexity:** Very High

**Implementation needs:**
- Broker API integration (Interactive Brokers, Alpaca, etc.)
- Order management system
- Real-time position tracking
- Security considerations (API keys, encryption)
- Trade execution logic

### 18. **AI/ML Features** (Weeks to months)
**Why:** Modern innovation

**Features:**
- Pattern recognition (head & shoulders, triangles, etc.)
- Anomaly detection
- Price prediction overlays
- Sentiment analysis integration
- Trade signal generation

**Complexity:** Very High

**Implementation needs:**
- ML model integration (TensorFlow, PyTorch)
- Training data pipeline
- Real-time inference
- Visualization of predictions
- Model updates and versioning

---

## 📊 Recommended Next Steps

### Quick Wins (1 week)
**For immediate UX improvement:**
1. **Mouse wheel zoom** (2-3 hours) - Game changer for interaction
2. **More keyboard shortcuts** (1 day) - Power user efficiency
3. **Auto-fit / reset view** (2-3 hours) - Quick navigation
4. **Color themes** (1-2 days) - Personalization

**Total:** ~1 week
**Impact:** High UX improvement with minimal effort

### Core Features (2-3 weeks)
**For professional functionality:**
1. **Moving Averages** (2-3 days) - First indicator, sets pattern
2. **RSI Indicator** (2 days) - Second indicator, proves architecture
3. **Chart type alternatives** (2-3 days) - Line/Area/Heikin-Ashi
4. **Multiple timeframes** (1-2 days) - Standard feature
5. **Save/load layouts** (2-3 days) - Professional workflow

**Total:** ~2-3 weeks
**Impact:** Professional-grade charting application

### Professional Polish (1-2 months)
**For production-ready application:**
1. All Core Features (above)
2. **Drawing tools** (3-5 days) - Trend lines, Fibonacci
3. **Settings panel** (2-3 days) - User customization
4. **Real-time updates** (3-5 days) - Live data streaming
5. **Performance profiling** (2-3 days) - Validate performance
6. **Unit tests** (1-2 weeks) - Code quality
7. **Documentation** (1 week) - User guide + API docs

**Total:** ~1-2 months
**Impact:** Production-ready, distributable application

---

## 🎯 Personal Recommendation

**Start with these 3 features** (in order):

### 1. Mouse Wheel Zoom (2-3 hours)
**Why first:**
- Biggest UX improvement for least effort
- Everyone expects this behavior
- Makes app feel modern and responsive

**ROI:** ⭐⭐⭐⭐⭐ (5/5)

### 2. Simple Moving Average (2-3 days)
**Why second:**
- Most requested indicator
- Proves indicator architecture works
- Opens path for all other indicators
- Forces you to think about overlay rendering

**ROI:** ⭐⭐⭐⭐⭐ (5/5)

### 3. Save/Load Layouts (2-3 days)
**Why third:**
- Professional workflow feature
- User configuration persistence
- Foundation for settings panel
- Makes app feel "complete"

**ROI:** ⭐⭐⭐⭐ (4/5)

**Total time:** ~1 week
**Result:** Professional charting app with indicators and great UX

---

## 💡 Implementation Tips

### For Indicators:
- Start with SMA (simplest)
- Create `Indicator` trait for all indicators
- Store calculated values in separate Vec
- Render as overlay or separate pane
- Make toggleable with keyboard

### For Mouse Wheel:
- Use `MouseWheel` events
- Zoom factor: 1.1x per tick
- Zoom toward cursor position (not center)
- Clamp visible_candle_count (min 10, max total)

### For Layouts:
- Use `serde` for serialization
- Store in TOML for readability
- Auto-save every 30 seconds
- Load on startup with fallback to defaults

### For Themes:
- Create `Theme` struct with all colors
- Store as resource
- Apply to all rendering systems
- Maybe use popular themes (Dracula, Nord, etc.)

---

## 📝 Notes

- Current performance: ~8µs crosshair, ready for 144fps+
- Current entity count: ~299 crosshair segments + chart elements
- Database: DuckDB with klines table
- Architecture: Bevy ECS with multi-pane layout
- Current features: Zoom, pan, resize, crosshair, volume toggle

---

## 🤝 Contributing Ideas

If you want to open-source this project, consider:
- Clean up code comments
- Add README with screenshots
- Write CONTRIBUTING.md
- Add LICENSE
- Set up CI/CD (GitHub Actions)
- Create example data for demo
- Record demo video

---

**Last updated:** Session 2025-11-03
**Current version:** v0.1.0 (prototype with 9 major features)
