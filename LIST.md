# Things to Work On

## 🔴 High Priority - Performance

1. **Incremental MA recalculation on lazy load**
   - Only calculate new values for the 100 newly loaded candles
   - Don't recalculate all 10,000+ existing values
   - Impact: Eliminates lag when scrolling through history

2. **Sliding window SMA algorithm**
   - Replace O(n × period) with O(n) using running sum
   - Subtract oldest value, add newest value
   - Impact: ~200× faster calculation for SMA-200

3. **Persistent indicator entities**
   - Keep entities alive, update transforms instead of despawn/spawn
   - Similar to crosshair implementation
   - Impact: Reduced entity churn, smoother rendering

---

## 🟡 Medium Priority - Indicator Features

4. **Indicator toggle key (M key)**
   - Press 'M' to show/hide all MAs
   - Or cycle through different MA combinations
   - Similar to 'V' key for volume pane

5. **EMA support**
   - Already implemented `calculate_ema()` but not used
   - Add EMA-12, EMA-26 (common for MACD)
   - Allow user to choose SMA vs EMA

6. **Bollinger Bands**
   - Upper/Lower bands: SMA ± (2 × standard deviation)
   - Render as semi-transparent filled area
   - Uses existing SMA calculation logic

7. **Indicator legend/labels**
   - Show "SMA-20", "SMA-50", "SMA-200" with color indicators
   - Display in top-left corner of Price pane
   - Makes it clear which line is which

8. **MA values in crosshair**
   - When hovering over a candle, show MA values at that point
   - "Price: 50000.00, SMA-20: 49950.50"
   - Already have the infrastructure in `update_crosshair()`

---

## 🟢 Low Priority - Separate Pane Indicators

9. **RSI (Relative Strength Index)**
   - Needs dedicated pane (0-100 range)
   - Uses `PaneType::Indicator` infrastructure already in place
   - Horizontal lines at 30 (oversold) and 70 (overbought)

10. **MACD (Moving Average Convergence Divergence)**
    - Needs dedicated pane (unbounded range)
    - MACD line, Signal line, Histogram
    - Popular momentum indicator

11. **Volume Profile / On-Balance Volume**
    - Alternative to volume bars
    - Could overlay on volume pane or separate pane

---

## 🔵 Polish - UX Improvements

12. **Indicator settings UI**
    - Panel to adjust MA periods (currently hardcoded 20, 50, 200)
    - Color picker for each MA
    - Toggle individual MAs on/off

13. **Indicator line thickness adjustment**
    - Currently hardcoded to 2px
    - Make configurable per indicator

14. **Anti-aliasing for MA lines**
    - Current sprite-based lines can look pixelated at angles
    - Consider using Bevy Gizmos or polyline crate
    - Smoother visual appearance

15. **MA line styles**
    - Solid, dashed, dotted
    - Help distinguish multiple MAs visually

---

## 🟣 Advanced Features

16. **Custom indicator framework**
    - Generic `Indicator` trait
    - Users can define custom formulas
    - Plugin-style architecture

17. **Indicator presets**
    - "Scalping": EMA-9, EMA-21
    - "Swing Trading": SMA-50, SMA-200
    - "Day Trading": EMA-12, EMA-26, RSI-14
    - Save/load preset configurations

18. **Performance profiling overlay**
    - Show calculation time, render time
    - FPS counter
    - Debug mode toggle

19. **Multi-timeframe analysis**
    - Show daily MA on 15m chart
    - Higher timeframe reference

20. **Indicator alerts**
    - Notify when MA crossover occurs (e.g., SMA-20 crosses above SMA-50)
    - Golden cross / Death cross detection

---

## 📊 Data / Infrastructure

21. **Optimize database queries**
    - Pre-calculate and store MA values in database
    - Load pre-calculated values instead of computing on-the-fly
    - Trade memory for speed

22. **Web API integration**
    - Fetch real-time data
    - Update candles and MAs live
    - WebSocket support

23. **Export functionality**
    - Export chart with indicators as image (PNG)
    - Export data with indicator values (CSV)

---

## Recommended Order

If prioritizing:

1. **Start with #4 (Toggle key)** - Quick win, immediately useful
2. **Then #7 (Indicator legend)** - Improves usability significantly
3. **Then #1 (Incremental recalc)** - Biggest performance gain
4. **Then #2 (Sliding window)** - Additional performance boost
5. **Then #6 (Bollinger Bands)** - Natural extension of SMA
6. **Then #9 (RSI)** - First separate pane indicator, proves architecture works
7. **Then #8 (MA in crosshair)** - Nice quality-of-life improvement

The rest can be prioritized based on your needs!
