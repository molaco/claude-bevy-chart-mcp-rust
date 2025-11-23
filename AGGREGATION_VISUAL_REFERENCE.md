# Flowsurface-Binance Aggregation - Visual Reference Guide

## 1. Time Boundary Computation - Visual Flow

```
Raw Trade Stream
├─ Trade: time=1000037, price=100.05, qty=1.0
├─ Trade: time=1000100, price=100.10, qty=2.0
└─ Trade: time=1000200, price=100.08, qty=1.5
    ↓
Apply Floor Division: (time / 60000) * 60000
├─ 1000037 → (16) * 60000 = 960000 ← BUCKET A
├─ 1000100 → (16) * 60000 = 960000 ← BUCKET A
└─ 1000200 → (16) * 60000 = 960000 ← BUCKET A
    ↓
Aggregate into Single Kline
{
    time: 960000,
    open: 100.05,      (first trade)
    high: 100.10,      (max of all trades)
    low: 100.05,       (min of all trades)
    close: 100.08,     (last trade)
    volume: (4.5, 0.0) (sum of quantities)
}
    ↓
Store in BTreeMap
BTreeMap {
    960000 → KlineDataPoint { kline, footprint },
    ...
}
```

## 2. Multiple Buckets - Timeline Visualization

```
Time Axis (milliseconds):
└──────── 0 ──────┴────── 60000 ──────┴───── 120000 ─────┴────── 180000 ────┘
         M1 Bucket A    M1 Bucket B     M1 Bucket C    M1 Bucket D

Trades on Timeline:
└────●───────●───────●─────┴────●──────●────●──┴────────────●─────●─────┘
     1000037  1000100 1000200    1060050 ...

Aggregation:
├─ Trades in [0, 60000): Bucket A
├─ Trades in [60000, 120000): Bucket B
├─ Trades in [120000, 180000): Bucket C
└─ Trades in [180000, 240000): Bucket D

Resulting Candles:
Candle A      Candle B      Candle C      Candle D
│  ●●●  │    │   ●●●  │    │           │    │  ●●  │
└─ 960000    └─ 1020000    └─ 1080000    └─ 1140000
```

## 3. Price Bucket Grouping - Vertical Bucketing

```
Order Book Level (Price Axis):
┌─────────────────────────────────────────┐
│                                         │
│ Price: 100.20 ─────────────────────────│  Bid/Ask: 100.20
│                                         │
│ Price: 100.10 ◆ ◆ ◆ ◆ ◆ ◆ ◆ ◆ ─────  │  Tick (0.01)
│ (Bucket)      Aggregated Trades        │
│                                         │
│ Price: 100.00 ◆ ◆ ◆ ◆ ──────────────  │
│ (Bucket)      Aggregated Trades        │
│                                         │
│ Price: 99.90  ──────────────────────   │
│                                         │
└─────────────────────────────────────────┘

Each trade rounded to nearest price:
Trade at 100.05 ───→ [Round to 100.00] ───→ Bucket: 100.00
Trade at 100.08 ───→ [Round to 100.10] ───→ Bucket: 100.10
Trade at 100.12 ───→ [Round to 100.10] ───→ Bucket: 100.10

Result:
Bucket 100.00: qty = 2.0 (from trades at 100.05)
Bucket 100.10: qty = 3.5 (from trades at 100.08, 100.12)
```

## 4. Global vs Local Alignment - Chart Panning Scenario

```
SCENARIO: 5:1 Aggregation, Panning from Left to Right

BEFORE FIX (Local Alignment - PROBLEMATIC):
┌─────────────────────────────────────────┐
│ Visible Candles: [0-4]                  │
│ Grouping: [0-4] together                │
│ Colors: [Green] (bullish group)         │
└─────────────────────────────────────────┘
         ↓ Pan right by 1 candle
┌─────────────────────────────────────────┐
│ Visible Candles: [1-5]                  │
│ Grouping: [1-5] together ← CHANGED!     │
│ Colors: [Red] (bearish group)           │
│ Result: COLOR FLICKERS! Gaps appear!    │
└─────────────────────────────────────────┘
         ↓ Pan right by 1 candle
┌─────────────────────────────────────────┐
│ Visible Candles: [2-6]                  │
│ Grouping: [2-6] together ← CHANGED AGAIN │
│ Colors: [Green] (bullish group)         │
│ Result: MORE FLICKERING!                │
└─────────────────────────────────────────┘

AFTER FIX (Global Alignment - CORRECT):
┌─────────────────────────────────────────┐
│ Visible Candles: [0-4]                  │
│ Global Grouping: [0-4] together         │
│ Colors: [Green]                         │
│ aligned_start = (0 / 5) * 5 = 0        │
└─────────────────────────────────────────┘
         ↓ Pan right by 1 candle
┌─────────────────────────────────────────┐
│ Visible Candles: [1-5]                  │
│ Global Grouping: Still [0-4]!           │
│ Colors: Still [Green]                   │
│ aligned_start = (1 / 5) * 5 = 0        │
│ Result: NO FLICKER! Smooth rendering!   │
└─────────────────────────────────────────┘
         ↓ Pan right by 1 candle
┌─────────────────────────────────────────┐
│ Visible Candles: [2-6]                  │
│ Global Grouping: Now [5-9]!             │
│ Colors: Changed once, not continuously  │
│ aligned_start = (2 / 5) * 5 = 0        │
│ Then: (5 / 5) * 5 = 5 (new group)      │
│ Result: Smooth transition! 60 FPS!      │
└─────────────────────────────────────────┘
```

## 5. Timeframe Conversion - Millisecond Mapping

```
Timeframe Enum          Milliseconds       Minutes        Hours    Days
─────────────────────────────────────────────────────────────────────────
MS100                   100               0.00167         0.00003   0
MS200                   200               0.00333         0.00006   0
MS300                   300               0.005           0.00008   0
MS500                   500               0.0083          0.00014   0
MS1000                  1,000             0.0167          0.000278  0
M1                      60,000            1               0.0167    0
M3                      180,000           3               0.05      0
M5                      300,000           5               0.083     0
M15                     900,000           15              0.25      0
M30                     1,800,000         30              0.5       0
H1                      3,600,000         60              1         0.0417
H2                      7,200,000         120             2         0.0833
H4                      14,400,000        240             4         0.167
H6                      21,600,000        360             6         0.25
H12                     43,200,000        720             12        0.5
D1                      86,400,000        1440            24        1

Usage Examples:
trade.time = 1234567890      (milliseconds since epoch)
timeframe = Timeframe::M5    (5 minutes)
aggr_time = 300000           (from to_milliseconds())
rounded = (1234567890 / 300000) * 300000 = 1234500000
```

## 6. Timezone-Aware Daily Candle Boundary

```
Scenario: Daily candle (D1) on Shanghai time (UTC+8)

UTC Timestamp: 1700000000000 (Nov 15, 2023, 04:26:40 UTC)
Shanghai Time: Nov 15, 2023, 12:26:40 CST (4 hours 26 minutes past midnight)

Step 1: Apply Offset (+28800 seconds = +8 hours)
        1700000000000 + 28800000 = 1700028800000
        Shanghai: Nov 15, 2023, 00:00:00 CST (midnight)

Step 2: Align to Day Boundary (86400000 ms)
        (1700028800000 / 86400000) * 86400000 = 1700028800000
        (Already aligned to midnight!)

Step 3: Convert Back to UTC (subtract offset)
        1700028800000 - 28800000 = 1700000000000
        Back to original UTC timestamp

Result:
Candle time: 1700000000000 (stored in UTC)
Represents:  Shanghai midnight to Shanghai midnight
Covers:      1699913600000 to 1700000000000 (in UTC)
             Noon previous day to noon current day (UTC perspective)

Key: Same source candles always aggregate together in Shanghai time,
     regardless of what hour it appears to be in UTC.
```

## 7. Label Step Selection Algorithm - Visual Decision Tree

```
INPUT: 
  - Visible time range: earliest, latest
  - Available horizontal space: labels_can_fit
  - Current timeframe: M5

┌─ Calculate timeframe_in_min = 5 (from to_milliseconds / 60000)
│
├─ Select step array based on timeframe:
│  ├─ If MS100-MS1000: Use MS_TIME_STEPS
│  ├─ If M1-M30: Use M1_TIME_STEPS, M3_TIME_STEPS, M5_TIME_STEPS
│  └─ If H1+: Use HOURLY_TIME_STEPS
│
├─ Test each step from largest to smallest:
│  ├─ Test 1440000 (24h): Does 86400000/1440000 >= 10 labels? → Yes! Use this
│  │  Result: ~60 labels per 24h period (one per 24 minutes)
│  ├─ If no: test 720000 (12h)
│  ├─ If no: test 480000 (8h)
│  └─ If no: test smaller...
│
└─ Align first label to step boundary:
   rounded_earliest = (earliest / selected_step) * selected_step
   First label appears at this aligned time

OUTPUT:
  - Label step: 1440000 ms (24 minutes for M5 chart)
  - First label time: aligned to step boundary
  - Subsequent labels: step by 1440000 ms
```

## 8. Price Label Generation - Wilkinson Algorithm Visualization

```
Input: Price range 100.00 to 105.50, 10 labels can fit

Step 1: Calculate range
  range = 105.50 - 100.00 = 5.50

Step 2: Calculate base (power of 10)
  log10(5.50) = 0.74
  floor(0.74) = 0
  base = 10^0 = 1.0

Step 3: Calculate ratio
  ratio = range / base = 5.50 / 1.0 = 5.50

Step 4: Select step based on ratio vs labels
  labels = 10
  5.50 ≈ labels * 0.5 → step = 0.5 * 1.0 = 0.5
  (But in this case, matches labels * 2.0 better)
  → step = 1.0

Step 5: Generate labels starting from aligned boundary
  Rounded highest = ceil(105.50 / 1.0) * 1.0 = 106.0
  Labels: 100, 101, 102, 103, 104, 105, 106
  
  Result: Nice round numbers, ~7 labels fit comfortably
```

## 9. Data Flow Diagram - From Trade to Screen

```
Exchange WebSocket/REST API
          ↓
    Raw Trade Data
    {time, price, qty, is_sell}
          ↓
  ┌─────────────────────┐
  │ AGGREGATION LAYER   │
  ├─────────────────────┤
  │ Time Bucketing:     │
  │ rounded_time =      │
  │ (time / 60000) * 60000
  │                     │
  │ Price Bucketing:    │
  │ grouped_price =     │
  │ round_to_tick(price)│
  └─────────────────────┘
          ↓
    BTreeMap<u64, Kline>
    {960000: {OHLC, volume}}
          ↓
  ┌─────────────────────┐
  │ RENDERING LAYER     │
  ├─────────────────────┤
  │ Query visible range │
  │ [earliest..latest]  │
  │                     │
  │ For each candle:    │
  │ x = interval_to_x() │
  │ y_o, y_h, y_l, y_c  │
  │   = price_to_y()    │
  │                     │
  │ Draw candle body &  │
  │ wicks at (x, y)     │
  └─────────────────────┘
          ↓
    Screen Pixels
    (with panning/zoom
     applied by ViewState)
```

## 10. Boundary Properties - Mathematical Guarantees

```
Given:
  - Formula: align(t, i) = (t / i) * i
  - i = aggregation interval
  - t = timestamp

Property 1: IDEMPOTENCE
  align(align(t, i), i) = align(t, i)
  
  Example:
  align(1000037, 60000) = 960000
  align(960000, 60000) = 960000  ✓ Same result

Property 2: TILING (No gaps/overlaps)
  ∀ t: exactly one bucket contains t
  
  Example with i=10:
  Bucket 0: [0, 10)    includes: 0,1,2,3,4,5,6,7,8,9
  Bucket 1: [10, 20)   includes: 10,11,12,13,14,15,16,17,18,19
  Bucket 2: [20, 30)   includes: 20,21,22,23,24,25,26,27,28,29
  
  No gaps, no overlaps ✓

Property 3: MONOTONICITY
  t1 < t2 → align(t1, i) ≤ align(t2, i)
  
  Example:
  align(5, 10) = 0
  align(15, 10) = 10
  0 ≤ 10 ✓

Property 4: GLOBAL CONSISTENCY
  Bucket boundaries independent of viewport
  
  Example:
  Visible [0-99] with i=10: Buckets 0-9
  Visible [50-99] with i=10: Still boundaries at multiples of 10
  Same candles aggregate same way ✓
```

## Summary Table: At a Glance

| Operation | Formula | Location | Complexity |
|-----------|---------|----------|-------------|
| Time alignment | `(t / i) * i` | `time.rs:209` | O(1) |
| To milliseconds | `minutes * 60_000` | `lib.rs:161` | O(1) |
| Price bucket | `price.round_to_side_step()` | `heatmap.rs:43` | O(1) |
| Bucket lookup | Binary search | `heatmap.rs:46` | O(log m) |
| Timezone align | Apply offset, align, revert | `forming_candle.rs:91-116` | O(1) |
| Label step | Select from array | `timeseries.rs:97` | O(log k) |
| Price labels | Wilkinson algorithm | `linear.rs:11` | O(1) |
| Global align | `(start / ratio) * ratio` | Chart rendering | O(1) |

---

This visual reference should help you understand how all the pieces fit together!
