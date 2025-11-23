# Flowsurface-Binance Aggregation Boundaries & Alignment - Documentation Index

## Overview

This comprehensive documentation set analyzes how the flowsurface-binance charting system computes, aligns, and manages aggregation boundaries across three critical dimensions:

1. **Time Boundaries** - Horizontal axis aggregation (candlestick periods)
2. **Bucket Alignment** - Vertical axis grouping (price levels)  
3. **Viewport Alignment** - Rendering consistency (global vs local positioning)

All three use the fundamental principle of **globally-aligned floor division** to ensure consistency, performance, and visual stability.

---

## Documents in This Set

### 1. AGGREGATION_BOUNDARIES_DETAILED_ANALYSIS.md
**Length**: ~530 lines | **Depth**: Comprehensive technical analysis

This is your **deep dive** document covering:
- Complete time boundary computation mechanism with examples
- Timeframe enum and millisecond conversion system
- Price bucketing via tick-size-aware rounding
- Timezone-aware daily candle alignment
- Global vs local alignment in chart rendering
- Label boundary calculation (time and price axes)
- Full data structure hierarchy
- Mathematical properties and guarantees
- Performance analysis for each component
- Testing and validation approaches
- Related commits and system evolution

**Best for**: Understanding the entire system, implementation details, advanced use cases

**Key sections**:
- Section 1: Time aggregation boundaries (the core formula)
- Section 3: Candle time boundary alignment (timezone handling)
- Section 4: Global vs local alignment (critical for rendering)
- Section 7: Key alignment properties (mathematical guarantees)

---

### 2. AGGREGATION_KEY_FINDINGS.md
**Length**: ~200 lines | **Depth**: Focused summary with quick reference

This is your **quick reference** document containing:
- Quick reference for the 5 core mechanisms
- File location map for all relevant code
- Critical mathematical properties
- Performance characteristics
- Real-world example (single trade through system)
- Integration points (inbound/outbound/rendering)
- Key commits in evolution
- Validation checklist
- Common misconceptions
- Dependencies and related systems

**Best for**: Quick lookup, understanding relationships, validating implementations

**Key sections**:
- "Quick Reference: How Boundaries Work" (5 mechanisms)
- "File Locations - Quick Map" (code reference)
- "Critical Properties" (mathematical guarantees)
- "Real-World Example: Single Trade" (end-to-end)

---

### 3. AGGREGATION_VISUAL_REFERENCE.md
**Length**: ~350 lines | **Depth**: Visual diagrams and illustrations

This is your **visual learning** document with:
- Trade flow diagrams
- Timeline visualizations
- Price bucket groupings
- Global vs local alignment comparison scenarios
- Timeframe conversion table
- Timezone-aware boundary visualizations
- Label selection algorithm decision tree
- Price label generation algorithm walkthrough
- Complete data flow diagram (trade to screen)
- Mathematical property visualizations
- Summary table of all operations

**Best for**: Understanding flow and relationships, explaining to others, visual learners

**Key sections**:
- Section 1-3: Fundamental aggregation flows
- Section 4: Global vs local alignment scenarios (most important for UX)
- Section 9: Complete data flow diagram
- Summary table: All operations at a glance

---

## How to Use These Documents

### Scenario 1: "I need to understand time boundaries"
1. Start with **KEY_FINDINGS** Section 1 (5-minute read)
2. Look at **VISUAL_REFERENCE** Section 1-2 (see examples)
3. Deep dive with **DETAILED_ANALYSIS** Section 1 (if needed)

### Scenario 2: "Why is the chart flickering when I pan?"
1. Go to **VISUAL_REFERENCE** Section 4 (see the problem/solution)
2. Read **DETAILED_ANALYSIS** Section 4 (understand global alignment)
3. Check **KEY_FINDINGS** Section 5 (see related commits)

### Scenario 3: "How do axis labels get positioned?"
1. Start with **VISUAL_REFERENCE** Section 7-8 (see algorithms)
2. Check **DETAILED_ANALYSIS** Section 5 (implementation details)
3. Validate with **KEY_FINDINGS** File Locations (find the code)

### Scenario 4: "I need to implement a similar system"
1. Read **KEY_FINDINGS** Section "Summary Statement" (core idea)
2. Study **DETAILED_ANALYSIS** Section 7 (properties to maintain)
3. Use **DETAILED_ANALYSIS** Section 9 Summary Table (API reference)
4. Verify against **KEY_FINDINGS** Validation Points

### Scenario 5: "I want to present this to the team"
1. Show **VISUAL_REFERENCE** Section 4 (engaging visual comparison)
2. Walk through **VISUAL_REFERENCE** Section 9 (data flow)
3. Reference **KEY_FINDINGS** Common Misconceptions (address concerns)

---

## Cross-References Between Documents

### The Core Formula: `(timestamp / interval) * interval`

| Document | Location | Context |
|----------|----------|---------|
| DETAILED_ANALYSIS | Section 1 | Core mechanism explanation |
| KEY_FINDINGS | Quick Reference 1 | Formula and example |
| VISUAL_REFERENCE | Section 1 | Flow diagram with trades |
| VISUAL_REFERENCE | Section 10 | Mathematical guarantee |

### Global vs Local Alignment

| Document | Location | Context |
|----------|----------|---------|
| DETAILED_ANALYSIS | Section 4 | Comprehensive explanation |
| KEY_FINDINGS | Quick Reference 5 | Formula comparison |
| VISUAL_REFERENCE | Section 4 | Before/after scenarios |
| KEY_FINDINGS | Section 5 | Related commits (430f254) |

### Timezone Handling

| Document | Location | Context |
|----------|----------|---------|
| DETAILED_ANALYSIS | Section 3 | Implementation walkthrough |
| VISUAL_REFERENCE | Section 6 | Shanghai time example |
| KEY_FINDINGS | Quick Reference 4 | Summary mechanism |

---

## Code Reference Map

### Time Aggregation
**File**: `/data/src/aggr/time.rs:209`
```rust
let rounded_time = (trade.time / aggr_time) * aggr_time;
```
- **Discussed in**: DETAILED_ANALYSIS S1, KEY_FINDINGS S1, VISUAL_REFERENCE S1-2

### Timeframe Conversion
**File**: `/exchange/src/lib.rs:152-164`
```rust
pub fn to_milliseconds(self) -> u64 { ... }
```
- **Discussed in**: DETAILED_ANALYSIS S1, KEY_FINDINGS S3, VISUAL_REFERENCE S5

### Price Bucketing
**File**: `/data/src/chart/heatmap.rs:42-68`
```rust
let grouped_price: Price = trade.price.round_to_side_step(trade.is_sell, step);
```
- **Discussed in**: DETAILED_ANALYSIS S2, KEY_FINDINGS S2, VISUAL_REFERENCE S3

### Candle Alignment + Timezone
**File**: `/backtesting/src/forming_candle.rs:91-133`
```rust
pub fn new_with_timezone(timeframe, timestamp, timezone_offset_seconds) { ... }
```
- **Discussed in**: DETAILED_ANALYSIS S3, KEY_FINDINGS S4, VISUAL_REFERENCE S6

### Label Step Selection
**File**: `/src/chart/scale/timeseries.rs:73-110`
```rust
fn calc_time_step(earliest, latest, labels_can_fit, timeframe) { ... }
```
- **Discussed in**: DETAILED_ANALYSIS S5, VISUAL_REFERENCE S7

### Price Labels
**File**: `/src/chart/scale/linear.rs:7-24`
```rust
fn calc_optimal_ticks(highest, lowest, labels_can_fit) { ... }
```
- **Discussed in**: DETAILED_ANALYSIS S5, VISUAL_REFERENCE S8

### Global Alignment (Rendering)
**Location**: Chart rendering layer (commit 430f254)
```rust
aligned_start = (start / ratio) * ratio
```
- **Discussed in**: DETAILED_ANALYSIS S4, KEY_FINDINGS S5, VISUAL_REFERENCE S4

---

## Key Concepts Glossary

### Aggregation Interval (aggr_time)
- **Definition**: The time period for grouping trades into candles
- **Example**: M1 = 60,000 ms, M5 = 300,000 ms
- **Found in**: All three documents (Section 1-5)

### Globally Aligned Boundary
- **Definition**: A boundary position that doesn't change based on viewport
- **Formula**: `(position / interval) * interval`
- **Critical for**: Preventing flickering during panning
- **Found in**: DETAILED_ANALYSIS S4, VISUAL_REFERENCE S4, KEY_FINDINGS S5

### Bucket (Bucket A, B, C...)
- **Definition**: A time interval containing multiple trades
- **Example**: [960000, 1020000) is Bucket A for M1 at time 960000
- **Found in**: VISUAL_REFERENCE S1-2, DETAILED_ANALYSIS S1

### Price Level (Price Bucket)
- **Definition**: A specific price rounded to nearest tick
- **Example**: 100.10 is a price bucket with tick size 0.01
- **Found in**: DETAILED_ANALYSIS S2, VISUAL_REFERENCE S3, KEY_FINDINGS S2

### Tick Size
- **Definition**: The minimum price increment
- **Example**: 0.01 for BTC, 0.0001 for micro contracts
- **Used for**: Price rounding in buckets
- **Found in**: DETAILED_ANALYSIS S2, VISUAL_REFERENCE S3, KEY_FINDINGS S2

### Timezone Offset
- **Definition**: Hours offset from UTC for daily candle alignment
- **Example**: UTC+8 = 28800 seconds = 8 hours
- **Used for**: Shanghai-local daily candles
- **Found in**: DETAILED_ANALYSIS S3, VISUAL_REFERENCE S6, KEY_FINDINGS S4

---

## Mathematical Guarantees (All Three Documents)

All boundary calculations maintain these properties:

1. **Idempotence**: Applying twice gives same result
2. **Global Consistency**: Independent of viewport position
3. **No Gaps/Overlaps**: Every millisecond belongs to exactly one bucket
4. **Monotonic**: Time never goes backward

**Where to find proofs**: DETAILED_ANALYSIS S7, VISUAL_REFERENCE S10

---

## Performance Characteristics (All Three Documents)

| Operation | Complexity | Found in |
|-----------|------------|----------|
| Boundary calculation | O(1) | DETAILED_ANALYSIS S8 |
| BTreeMap insertion | O(log n) | DETAILED_ANALYSIS S8 |
| Price bucket lookup | O(log m) | DETAILED_ANALYSIS S8 |
| Label selection | O(log k) | DETAILED_ANALYSIS S8 |
| Rendering impact | 60+ FPS (global) | DETAILED_ANALYSIS S4, VISUAL_REFERENCE S4 |

---

## Common Questions Answered

| Question | Best Source |
|----------|------------|
| What is the core aggregation formula? | KEY_FINDINGS S1 |
| How are prices grouped? | VISUAL_REFERENCE S3 |
| Why does panning cause flickering? | VISUAL_REFERENCE S4 (before fix) |
| How are axis labels positioned? | VISUAL_REFERENCE S7-8 |
| What's the difference between global and local alignment? | DETAILED_ANALYSIS S4 |
| How does timezone affect candle boundaries? | VISUAL_REFERENCE S6 |
| What are the mathematical guarantees? | DETAILED_ANALYSIS S7, VISUAL_REFERENCE S10 |
| Where is each component implemented? | KEY_FINDINGS File Locations |
| What commits improved the system? | KEY_FINDINGS Section 5 |

---

## Recommended Reading Order

### For New Team Members (2-3 hours)
1. KEY_FINDINGS - Quick overview (30 min)
2. VISUAL_REFERENCE - Understand flows (45 min)
3. DETAILED_ANALYSIS - Deep dive on sections of interest (60+ min)

### For Implementation (4-5 hours)
1. KEY_FINDINGS - Get the summary (30 min)
2. DETAILED_ANALYSIS - Each relevant section (2-3 hours)
3. Code review + validation (1-2 hours)

### For Presentation (1-2 hours)
1. VISUAL_REFERENCE S4 - Most important chart insight (20 min)
2. VISUAL_REFERENCE S9 - Data flow overview (20 min)
3. KEY_FINDINGS - Talking points (40+ min)

---

## Testing & Validation

All concepts can be validated:
- See **KEY_FINDINGS** Validation Points section
- See **DETAILED_ANALYSIS** Section 10 Testing and Validation
- Verify each with code in `/data/src/aggr/time.rs`, `/exchange/src/lib.rs`, etc.

---

## Document Statistics

| Document | Lines | Sections | Code Examples | Diagrams |
|----------|-------|----------|---|----------|
| DETAILED_ANALYSIS | 532 | 11 | 40+ | 5+ |
| KEY_FINDINGS | 200 | 7 | 15+ | 2+ |
| VISUAL_REFERENCE | 356 | 10 | 20+ | 25+ |
| **TOTAL** | **1088** | **28** | **75+** | **32+** |

---

## Last Updated

These documents were created November 22, 2025, based on:
- Current codebase in `/home/molaco/Documents/flowsurface-binance`
- Latest commit: c216225 (feat: dynamic entity pool growth)
- Related improvements: 430f254 (global alignment for flicker-free panning)

---

## Questions or Clarifications?

If you need:
- **Implementation details**: See DETAILED_ANALYSIS
- **Quick answers**: See KEY_FINDINGS
- **Visual understanding**: See VISUAL_REFERENCE
- **Code location**: See KEY_FINDINGS File Locations Map

All documents are cross-referenced and complementary.
