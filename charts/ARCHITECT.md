# CHARTS CRATE ARCHITECTURE

```
CHARTS CRATE ARCHITECTURE
═════════════════════════════════════════════════════════════════════════════

📦 charts/
│
├─ 📄 Cargo.toml ──────────────────────── Dependencies: bevy, duckdb, chat-ui
│
├─ 📄 DOCS.md (662 lines) ───────────── Architecture & optimization docs
│
└─ 📁 src/
   │
   ├─ 🎯 main.rs (320 lines) ────────────────────────────────────────────────┐
   │    ├─ App initialization (Bevy + Plugins)                               │
   │    ├─ System registration & execution order                             │
   │    ├─ FPS counter (setup + update)                                      │
   │    ├─ BRP (Bevy Remote Protocol) for MCP                                │
   │    └─ Screenshot handlers (keypress + BRP)                              │
   │                                                                          │
   ├─ 📊 types.rs (1122 lines) ⭐ FOUNDATION ────────────────────────────────┤
   │    │                                                                     │
   │    ├─ 🗃️  DATA STRUCTURES                                               │
   │    │   ├─ Candle         → OHLCV + timestamp                            │
   │    │   ├─ ChartSpace     → Coordinate transformations                   │
   │    │   ├─ Pane/PaneId    → Multi-pane viewport regions                  │
   │    │   ├─ MovingAverage  → Indicator with caching                       │
   │    │   └─ Chart          → Main state container                         │
   │    │                                                                     │
   │    ├─ 🌐 RESOURCES (Global State)                                       │
   │    │   ├─ ChartDatabase        → DuckDB connection                      │
   │    │   ├─ InteractionState     → Mouse, dragging, resizing              │
   │    │   ├─ ChartGrid/ChartAxes  → Grid/axis config                       │
   │    │   ├─ Crosshair/CrosshairEntities → Crosshair state                 │
   │    │   └─ VolumeToggleState    → Volume visibility                      │
   │    │                                                                     │
   │    ├─ 🏷️  COMPONENTS (Entity Markers)                                   │
   │    │   ├─ CandlestickWick/Body → Candle entities                        │
   │    │   ├─ VolumeBar            → Volume entities                        │
   │    │   ├─ PriceElement/VolumeElement/IndicatorElement                   │
   │    │   ├─ GridElement          → Grid/axis entities                     │
   │    │   └─ CrosshairElement     → Crosshair entities                     │
   │    │                                                                     │
   │    ├─ 🧮 CALCULATIONS                                                    │
   │    │   ├─ calculate_sma()          → Full O(n)                          │
   │    │   ├─ calculate_sma_append()   → Incremental ✅                     │
   │    │   ├─ calculate_sma_prepend()  → Incremental ✅                     │
   │    │   ├─ calculate_ema()          → Exponential MA                     │
   │    │   └─ calculate_pane_layouts() → Layout math                        │
   │    │                                                                     │
   │    └─ ✅ TESTS (12 unit tests)                                          │
   │                                                                          │
   ├─ 🎨 rendering.rs (901 lines) ───────────────────────────────────────────┤
   │    │                              ↑ depends on types.rs                 │
   │    ├─ render_candlesticks()     → Spawn wick + body entities            │
   │    ├─ render_volume_bars()      → Spawn volume bar entities             │
   │    ├─ render_grid_and_axes()    → Grid + labels + borders               │
   │    ├─ render_moving_averages()  → Gizmo lines (GPU, no entities) ✅     │
   │    ├─ update_crosshair()        → Update persistent crosshair           │
   │    ├─ init_crosshair()          → Create crosshair entities             │
   │    └─ format_volume()           → Helper (K/M/B formatting)             │
   │                                                                          │
   ├─ 🖱️  interaction.rs (439 lines) ────────────────────────────────────────┤
   │    │                              ↑ depends on types.rs                 │
   │    ├─ handle_mouse_input()      → Pan, zoom, pane resize                │
   │    ├─ check_lazy_load()         → Prepend/append candles at edges       │
   │    ├─ toggle_volume_pane()      → 'V' key toggle                        │
   │    └─ toggle_sma_indicators()   → '1'/'2'/'3'/'S' keys                  │
   │                                                                          │
   ├─ 📐 ui_layout.rs (92 lines) ────────────────────────────────────────────┤
   │    ├─ setup_split_layout()      → Chart 70% | Chat 30%                  │
   │    └─ reparent_chat_to_container() → Hierarchy fix                      │
   │                                                                          │
   └─ 📸 screenshot.rs (22 lines) ───────────────────────────────────────────┘
        └─ take_screenshot()          → Capture window to PNG


SYSTEM EXECUTION ORDER
═════════════════════════════════════════════════════════════════════════════

   [Startup]
       ↓
   setup_split_layout()  ─────→ Create UI containers (70/30 split)
       ↓
   setup()  ──────────────────→ Load DB → Load 500 candles → Create Chart
       ↓
   setup_fps_counter()  ───────→ Create FPS display text
       ↓
   [PostStartup]
       ↓
   reparent_chat_to_container() → Move chat-ui into layout
       ↓
   [Update Loop @ 60-100 FPS] ───────────────────────────────────────┐
       │                                                              │
       ├─→ update_chart_context()  ──→ Update AI chat context        │
       │                                                              │
       ├─→ handle_mouse_input() ─────→ Pan/zoom/resize panes         │
       │          ↓ (chained)                                         │
       │   update_crosshair() ────────→ Position crosshair           │
       │                                                              │
       ├─→ toggle_volume_pane() ─────→ 'V' key handler               │
       ├─→ toggle_sma_indicators() ──→ '1'/'2'/'3'/'S' keys          │
       ├─→ check_lazy_load() ────────→ Auto-load data at edges       │
       ├─→ screenshot_on_keypress() ─→ 'F' key handler               │
       │                                                              │
       └─→ [RENDERING CHAIN - chained execution]                     │
              ↓                                                       │
           render_grid_and_axes() ───→ Grid + labels + borders       │
              ↓                                                       │
           render_candlesticks() ────→ Spawn candle entities (⚠️)    │
              ↓                                                       │
           render_moving_averages() ─→ Draw MA lines (Gizmos ✅)     │
              ↓                                                       │
           render_volume_bars() ─────→ Spawn volume entities (⚠️)    │
              ↓                                                       │
           reset_redraw_flag() ──────→ Clear needs_redraw            │
              ↓                                                       │
           [Back to Update Loop] ────────────────────────────────────┘


DATA FLOW DIAGRAM
═════════════════════════════════════════════════════════════════════════════

┌──────────────────────┐
│   DuckDB Database    │  OHLCV historical data
│   (ChartDatabase)    │
└──────────┬───────────┘
           │ load_candles()
           ↓
┌──────────────────────────────────────────────────────┐
│                  Chart Resource                      │  Main state
│  ┌──────────────┬──────────────┬──────────────────┐ │
│  │ candles: Vec │ visible_start│ panes: Vec<Pane> │ │
│  │  [500-1000]  │ visible_count│  ├─ Price        │ │
│  │              │              │  ├─ Volume       │ │
│  │              │              │  └─ Indicators   │ │
│  └──────────────┴──────────────┴──────────────────┘ │
│  ┌─────────────────────────────────────────────────┐│
│  │ Moving Averages: SMA-20, SMA-50, SMA-200        ││
│  │  ├─ Sliding window calculation (O(n))           ││
│  │  └─ Incremental append/prepend ✅                ││
│  └─────────────────────────────────────────────────┘│
└───────────────────────┬──────────────────────────────┘
                        │
            ┌───────────┼───────────┐
            ↓           ↓           ↓
    ┌──────────┐ ┌────────────┐ ┌─────────────┐
    │  Input   │ │  Rendering │ │ Interaction │
    │  Layer   │ │   Layer    │ │    State    │
    └──────────┘ └────────────┘ └─────────────┘
         │             │               │
         │ Mouse/KB    │ Entities      │ Pan/Zoom
         │ Events      │ + Gizmos      │ Resize
         ↓             ↓               ↓
    ┌────────────────────────────────────────────┐
    │         InteractionState                   │
    │  ├─ mouse_pos                              │
    │  ├─ is_dragging                            │
    │  ├─ is_resizing_pane                       │
    │  └─ resize_pane_index                      │
    └────────────────────────────────────────────┘
                        │
                        ↓ Modifies
            ┌───────────────────────┐
            │  Chart.visible_start  │ → Triggers needs_redraw=true
            │  Chart.visible_count  │
            │  Pane.height_percent  │
            └───────────────────────┘
                        │
                        ↓ Triggers rendering
            ┌───────────────────────┐
            │  World Entities       │
            │  ├─ 500+ Candlesticks │ ✅ In-place updates (optimized 2025-11)
            │  ├─ 200+ Volume bars  │ ✅ In-place updates (optimized 2025-11)
            │  ├─ Grid/axes         │
            │  └─ Crosshair (~40)   │ ✅ Persistent + visibility caching
            │                       │
            │  Gizmos (non-entity)  │
            │  └─ MA lines (3)      │ ✅ GPU-accelerated, optimized
            └───────────────────────┘


MULTI-PANE ARCHITECTURE
═════════════════════════════════════════════════════════════════════════════

Window (1600 x 900)
╔═══════════════════════════════════════════════════════════════════════════╗
║ ┌──────────────────────────────────────┬──────────────────────────────┐   ║
║ │  Chart Container (70% = 1120px)     │  Chat Container (30% = 480px)│   ║
║ │ ┌──────────────────────────────────┐ │ ┌──────────────────────────┐ │   ║
║ │ │  🎯 Pane: PRICE (60% height)     │ │ │     Chat UI (chat-ui)    │ │   ║
║ │ │  ├─ Candlesticks                 │ │ │   - Message history      │ │   ║
║ │ │  ├─ Moving Averages (SMA)        │ │ │   - User input           │ │   ║
║ │ │  ├─ Grid (horizontal + vertical) │ │ │   - AI responses         │ │   ║
║ │ │  ├─ Y-axis labels (right)        │ │ │   - Chart context        │ │   ║
║ │ │  └─ Crosshair + OHLCV info box   │ │ └──────────────────────────┘ │   ║
║ │ └─────────────────────[ Border ]───┘ │                              │   ║
║ │ ╌╌╌╌╌╌╌╌╌╌ GAP (20px) ╌╌╌╌╌╌╌╌╌╌╌╌╌ │  Integrated via:             │   ║
║ │ ┌──────────────────────────────────┐ │  ui_layout.rs                │   ║
║ │ │  📊 Pane: VOLUME (30% height)    │ │  reparent system             │   ║
║ │ │  ├─ Volume bars (color coded)    │ │                              │   ║
║ │ │  ├─ Grid (horizontal only)       │ │                              │   ║
║ │ │  ├─ Y-axis labels (right)        │ │                              │   ║
║ │ │  └─ Toggle: 'V' key              │ │                              │   ║
║ │ └─────────────────────[ Border ]───┘ │                              │   ║
║ │ ╌╌╌╌╌╌╌╌╌╌ GAP (20px) ╌╌╌╌╌╌╌╌╌╌╌╌╌ │                              │   ║
║ │ ┌──────────────────────────────────┐ │                              │   ║
║ │ │  📈 Pane: INDICATOR (10% height) │ │                              │   ║
║ │ │  ├─ Future: RSI, MACD, etc.      │ │                              │   ║
║ │ │  └─ Currently unused             │ │                              │   ║
║ │ └──────────────────────────────────┘ │                              │   ║
║ │                                      │                              │   ║
║ │ Common X-axis (time) across panes    │                              │   ║
║ │ Independent Y-axis per pane          │                              │   ║
║ └──────────────────────────────────────┴──────────────────────────────┘   ║
╚═══════════════════════════════════════════════════════════════════════════╝


KEY RELATIONSHIPS & DEPENDENCIES
═════════════════════════════════════════════════════════════════════════════

main.rs  ────────────────┬─────────────────┬─────────────────┐
  (orchestrator)         │                 │                 │
                         ↓                 ↓                 ↓
                    types.rs         ui_layout.rs      screenshot.rs
                   (foundation)       (standalone)      (standalone)
                         ↑
            ┌────────────┼────────────┐
            │                         │
        rendering.rs            interaction.rs
      (visual output)          (user input)
            │                         │
            └──────────┬──────────────┘
                       ↓
              Shared dependencies:
            Chart, Candle, Pane,
           ChartSpace, Coordinate
          transformations, State


PERFORMANCE CHARACTERISTICS
═════════════════════════════════════════════════════════════════════════════

Component              Mechanism                      Status
─────────────────────────────────────────────────────────────────────────────
SMA Calculation        Sliding window O(n)            ✅ Optimized
Lazy Loading           Incremental append/prepend     ✅ Optimized
MA Rendering           Gizmos (GPU, no entities)      ✅ Optimized
Crosshair              Visibility state caching       ✅ Optimized (NEW: -83% ECS ops)
Candlesticks           In-place entity updates        ✅ Optimized (NEW: -97% spawn/despawn)
Volume Bars            In-place entity updates        ✅ Optimized (NEW: -97% spawn/despawn)
Grid/Axes              Entity spawn/despawn           ⚠️  Low priority

RECENT OPTIMIZATIONS (2025-11):
• Candlestick rendering: In-place Transform/Sprite updates with HashMap tracking
  - Before: 200+ despawn/spawn operations per redraw
  - After: 0-4 operations during pan, only updates on zoom edges
  - Impact: 30-40% FPS improvement during pan/zoom

• Crosshair visibility: State-change-only ECS updates with caching
  - Before: 38 visibility queries × 60 FPS = 2,280 ops/sec
  - After: Updates only on state changes (~5-10/sec) = ~380 ops/sec
  - Impact: 10-15% FPS improvement, 83% reduction in ECS overhead

FPS: 60-100 (uncapped, no VSync) - improved with recent optimizations
Visible candles: 10-10000 (zoom range)
Lazy load threshold: 20 candles from edge
Database: DuckDB with connection pooling


INTERACTION CONTROLS
═════════════════════════════════════════════════════════════════════════════

Mouse:
  🖱️  Left Drag      → Pan chart left/right
  🖱️  Scroll Wheel   → Zoom in/out (10-10000 candles)
  🖱️  Drag Gap       → Resize pane heights

Keyboard:
  ⌨️  1/2/3 keys     → Toggle SMA-20/50/200
  ⌨️  S key          → Toggle all SMAs
  ⌨️  V key          → Toggle volume pane
  ⌨️  F key          → Take screenshot

Auto-features:
  🔄 Lazy Loading    → Auto-load data at viewport edges
  ✨ Crosshair       → Follows mouse with OHLCV info


EXTERNAL INTEGRATIONS
═════════════════════════════════════════════════════════════════════════════

┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  Chat-UI Crate  │────→│  Charts (this)   │────→│  DuckDB Database│
│  (chat input)   │     │  (visualization) │     │  (OHLCV data)   │
└─────────────────┘     └──────────────────┘     └─────────────────┘
        ↑                        ↓
        │                        │
        │              ┌─────────┴─────────┐
        │              │  BRP (Remote)     │
        └──────────────│  MCP Integration  │
                       │  - Screenshot     │
                       │  - Context        │
                       └───────────────────┘
```

## Summary

This architecture map provides a comprehensive visual overview of the charts crate including:

- **Module hierarchy** with line counts and responsibilities
- **System execution order** from startup through update loop
- **Data flow** from database through rendering
- **Multi-pane layout** structure (Price/Volume/Indicator)
- **Dependencies** between modules
- **Performance characteristics** with optimization status
- **Integration points** with chat-ui and external systems

The architecture is well-organized with clear separation of concerns: `types.rs` provides the foundation, `rendering.rs` handles visuals, `interaction.rs` manages input, and `main.rs` orchestrates everything together.
