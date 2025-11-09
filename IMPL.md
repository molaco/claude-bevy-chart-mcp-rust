# Chat Interface Implementation Plan

## Overview

This document outlines the plan to build a chat interface for chart analysis using Bevy UI and the `bevy_ui_text_input` plugin, developed as a separate crate for testing before integration.

**Approach**: Build chat UI as standalone library → Test independently → Integrate with chart app

**Timeline**: 3-4 days total (2 days dev, 1 day Claude integration, 0.5 day integration)

---

## Project Structure

```
bevy-migration/
├── Cargo.toml              # Workspace root
├── charts/                 # Existing chart application
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── chat-ui/                # NEW: Chat interface library
│   ├── Cargo.toml
│   ├── README.md
│   ├── examples/
│   │   ├── simple.rs       # Basic text input test
│   │   ├── messages.rs     # Message display test
│   │   └── full.rs         # Complete chat test
│   └── src/
│       ├── lib.rs          # Plugin + re-exports
│       ├── components.rs   # Components & resources
│       ├── layout.rs       # UI layout systems
│       ├── input.rs        # Text input handling
│       └── messages.rs     # Message display & scrolling
└── IMPL.md                 # This file
```

---

## Phase 0: Workspace Setup (30 minutes)

### Task 0.1: Create Workspace Root Configuration

**File**: `/Cargo.toml` (workspace root)

```toml
[workspace]
members = [
    "charts",
    "chat-ui",
]
resolver = "2"

[workspace.dependencies]
bevy = "0.17"
bevy_ui_text_input = "0.2"
```

**Commands**:
```bash
cd /home/molaco/Documents/bevy-migration
# Modify existing Cargo.toml to add workspace
```

### Task 0.2: Update Charts Crate

**File**: `charts/Cargo.toml`

Update dependencies to use workspace versions:
```toml
[dependencies]
bevy = { workspace = true }
# Keep other existing dependencies
```

### Task 0.3: Create Chat UI Crate

```bash
cd /home/molaco/Documents/bevy-migration
cargo new --lib chat-ui
```

**File**: `chat-ui/Cargo.toml`

```toml
[package]
name = "chat-ui"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { workspace = true }
bevy_ui_text_input = { workspace = true }

[dev-dependencies]
bevy = { workspace = true }

[[example]]
name = "simple"
path = "examples/simple.rs"

[[example]]
name = "messages"
path = "examples/messages.rs"

[[example]]
name = "full"
path = "examples/full.rs"
```

### Task 0.4: Verify Workspace

```bash
cargo build --workspace
```

**Expected**: Both crates compile successfully

---

## Phase 1: Core Components (2 hours)

### Task 1.1: Define Data Structures

**File**: `chat-ui/src/components.rs`

Create:
- `ChatState` resource (stores messages, waiting state)
- `ChatMessage` struct (content, is_user, timestamp)
- `ChatInputField` component (marker for text input)
- `MessageContainer` component (marker for message list)
- `ChatMessageEntity` component (tracks message index)
- `LoadingIndicator` component (marker for "thinking..." message)

**Lines**: ~40 lines

### Task 1.2: Create Plugin Structure

**File**: `chat-ui/src/lib.rs`

Create:
- `ChatUiPlugin` struct
- Plugin implementation with system registration
- Module declarations
- Public prelude module for easy imports

**Lines**: ~30 lines

### Task 1.3: Test Compilation

```bash
cargo build --package chat-ui
```

**Expected**: Compiles successfully with no UI yet

---

## Phase 2: Text Input (2 hours)

### Task 2.1: Create Simple Input Example

**File**: `chat-ui/examples/simple.rs`

Build minimal example:
- Spawn camera
- Spawn single TextInputNode
- Handle SubmitText events
- Log submitted text

**Purpose**: Verify bevy_ui_text_input works

**Lines**: ~40 lines

### Task 2.2: Test Simple Input

```bash
cargo run --package chat-ui --example simple
```

**Verify**:
- ✅ Text input appears on screen
- ✅ Can type text
- ✅ Cursor visible and blinking
- ✅ Arrow keys move cursor
- ✅ Backspace works
- ✅ Enter logs to console
- ✅ Ctrl+C/V works (clipboard)

### Task 2.3: Implement Input Handler

**File**: `chat-ui/src/input.rs`

Create `handle_text_submit` system:
- Read SubmitText events
- Query TextInputContents
- Add message to ChatState
- Clear input field
- Maintain focus on input
- Log user messages

**Lines**: ~40 lines

---

## Phase 3: UI Layout (3 hours)

### Task 3.1: Design Layout Structure

Create flexbox layout:
```
Root (Column, 100% x 100%)
├── Header (Fixed 50px height)
├── Message Area (Flex-grow: 1, Scrollable)
└── Input Area (Fixed 120px height)
```

### Task 3.2: Implement Layout System

**File**: `chat-ui/src/layout.rs`

Create `setup_chat_ui` system:
- Spawn camera
- Create root container (full screen, column direction)
- Add header with title text
- Add scrollable message container
  - Set overflow: clip_y
  - Add ScrollPosition component
  - Add MessageContainer marker
- Add input area at bottom
  - Fixed height
  - Border styling
  - Spawn TextInputNode

**Lines**: ~120 lines

### Task 3.3: Test Layout

```bash
cargo run --package chat-ui --example full
```

**Verify**:
- ✅ Header displays "Chat Interface Test"
- ✅ Message area is empty but visible
- ✅ Input field at bottom with border
- ✅ Can click and type in input
- ✅ Layout is responsive to window resize

---

## Phase 4: Message Display (3 hours)

### Task 4.1: Implement Message Rendering

**File**: `chat-ui/src/messages.rs`

Create `update_message_display` system:
- Detect when ChatState.messages changes
- Query MessageContainer entity
- Count existing message entities
- Spawn new message entities for any new messages
- Style messages with clear user/assistant differentiation
  - Option A: Terminal-style prefixes ("You: " / "Assistant: ")
  - Option B: Color-coded + aligned (blue right / green left)
- Add padding, margins, borders

**Lines**: ~80 lines

### Task 4.2: Create Messages Example

**File**: `chat-ui/examples/messages.rs`

Create example that:
- Shows layout
- Pre-populates with sample messages
- Tests message display without input

**Lines**: ~60 lines

### Task 4.3: Test Message Display

```bash
cargo run --package chat-ui --example messages
```

**Verify**:
- ✅ Sample messages appear
- ✅ User/assistant messages clearly differentiated
- ✅ Messages have proper spacing
- ✅ Text is readable
- ✅ Chosen styling approach is consistent

### Task 4.4: Implement Auto-scroll

**File**: `chat-ui/src/messages.rs`

Create `auto_scroll_to_bottom` system:
- Query MessageContainer with Changed<Children>
- Calculate total content height
- Get container height
- Set ScrollPosition to show bottom

**Lines**: ~25 lines

### Task 4.5: Test Auto-scroll

Modify messages example to add messages on timer

**Verify**:
- ✅ Automatically scrolls to newest message
- ✅ Can manually scroll up to read history
- ✅ New messages bring scroll back to bottom

---

## Phase 5: Full Chat Flow (2 hours)

### Task 5.1: Connect Input to Messages

**File**: `chat-ui/examples/full.rs`

Complete example:
- Use ChatUiPlugin
- Add mock response system (waits 2 seconds, adds fake response)
- Test full conversation flow

**Lines**: ~50 lines

### Task 5.2: Add Loading Indicator

Update `messages.rs`:
- When ChatState.is_waiting = true
- Spawn LoadingIndicator entity
- Show "Claude is thinking..." message
- Remove when response arrives

**Lines**: ~30 lines addition

### Task 5.3: Test Full Flow

```bash
cargo run --package chat-ui --example full
```

**Verify**:
- ✅ Type message and press Enter
- ✅ User message appears (blue, right)
- ✅ "Claude is thinking..." appears
- ✅ After 2 seconds, mock response appears (green, left)
- ✅ Can continue conversation
- ✅ Auto-scrolls to latest message
- ✅ Input clears after send
- ✅ Input stays focused

---

## Phase 6: Claude SDK Integration (1 day)

### Task 6.1: Add Claude SDK Dependency

**File**: `chat-ui/Cargo.toml`

```toml
[dependencies]
claude-agent-sdk-rust = { path = "../../claude-agent-sdk-rust" }
tokio = { version = "1", features = ["full"] }
```

### Task 6.2: Create Claude Handler System

**File**: `chat-ui/src/claude.rs`

Create:
- Resource to hold ClaudeSDKClient
- `send_to_claude` system (triggered on new user message)
- `receive_from_claude` system (polls for responses)
- Error handling for Claude API failures

**Lines**: ~80 lines

### Task 6.3: Add Claude Module to Plugin

**File**: `chat-ui/src/lib.rs`

Add:
- `mod claude;`
- Claude systems to Update schedule
- ClaudeSDKClient initialization

### Task 6.4: Test with Real Claude

Update `examples/full.rs` to use real Claude SDK

**Verify**:
- ✅ Message sent to Claude API
- ✅ Response received and displayed
- ✅ Error handling works (network issues, etc.)
- ✅ Multiple back-and-forth messages work

---

## Phase 7: Styling & Polish (3 hours)

### Task 7.1: Add Rich Text Support (Optional)

If needed for code formatting:

```toml
[dependencies]
bevy_simple_rich_text = "0.1"
```

Add BBCode-style formatting for:
- Bold text
- Code blocks
- Different colors

**Lines**: ~50 lines

### Task 7.2: Improve Visual Design

Polish the existing styling:
- Refine color scheme and contrast
- Add border highlights on input focus
- Improve message spacing and padding
- Add visual feedback (hover states, focus indicators)
- Consider smooth scrolling animations (optional)
- Improve overall visual hierarchy

**Lines**: ~40 lines

### Task 7.3: Add Keyboard Shortcuts

Additional features:
- Escape to clear input
- Ctrl+L to clear all messages
- Ctrl+K to scroll to top

**Lines**: ~40 lines

### Task 7.4: Add Configuration

**File**: `chat-ui/src/config.rs`

Create `ChatUiConfig` resource:
- Colors (background, text, borders, accents)
- Dimensions (input height, message padding, header height)
- Behavior (auto-scroll enabled, max messages, show timestamps)
- Typography (font size, line height)

**Lines**: ~60 lines

---

## Phase 8: Integration with Chart App (4 hours)

### Task 8.1: Add Chat UI Dependency

**File**: `charts/Cargo.toml`

```toml
[dependencies]
chat-ui = { path = "../chat-ui" }
```

### Task 8.2: Modify Chart Layout

**File**: `charts/src/main.rs` (or new `charts/src/layout.rs`)

Change layout from full-screen chart to:
```
Root (Row, 100% x 100%)
├── Chart Area (60% width)
└── Chat Area (40% width)
```

Override chat-ui's `setup_chat_ui` with custom layout

**Lines**: ~60 lines

### Task 8.3: Add Claude Context

Enhance Claude integration to:
- Include chart data in context
- Send screenshot with first message
- Parse chart-specific queries

**File**: `charts/src/chat_integration.rs`

**Lines**: ~100 lines

### Task 8.4: Test Integrated App

```bash
cargo run --package charts
```

**Verify**:
- ✅ Chart renders on left (60%)
- ✅ Chat interface on right (40%)
- ✅ Can interact with both independently
- ✅ Chat can query chart data
- ✅ Performance is acceptable

---

## Phase 9: Testing & Refinement (3 hours)

### Task 9.1: Manual Testing

Test all features:
- Text input edge cases (very long messages, empty messages, special chars)
- Message display (many messages, very long messages)
- Scrolling (manual scroll, auto-scroll, scroll position)
- Claude integration (errors, timeouts, streaming)
- Layout responsiveness (window resize, different sizes)

### Task 9.2: Performance Testing

Monitor:
- FPS while typing
- Memory usage with 100+ messages
- Compile times
- Message render performance

### Task 9.3: Bug Fixes

Address any issues found in testing

### Task 9.4: Documentation

Create:
- `chat-ui/README.md` - Usage instructions
- Code comments for public APIs
- Example usage in docs

---

## Phase 10: Final Polish (2 hours)

### Task 10.1: Clean Up Code

- Remove debug logging
- Add proper error messages
- Format all code (`cargo fmt`)
- Run clippy (`cargo clippy`)

### Task 10.2: Final Testing

Run all examples one more time:
```bash
cargo run --package chat-ui --example simple
cargo run --package chat-ui --example messages
cargo run --package chat-ui --example full
cargo run --package charts
```

### Task 10.3: Git Commit

Create clear git history:
```bash
git add chat-ui/
git commit -m "feat: add chat-ui library crate with bevy_ui_text_input"

git add charts/
git commit -m "feat: integrate chat UI with chart application"
```

---

## Timeline Summary

| Phase | Duration | Cumulative |
|-------|----------|------------|
| 0. Workspace Setup | 0.5 hours | 0.5h |
| 1. Core Components | 2 hours | 2.5h |
| 2. Text Input | 2 hours | 4.5h |
| 3. UI Layout | 3 hours | 7.5h |
| 4. Message Display | 3 hours | 10.5h |
| 5. Full Chat Flow | 2 hours | 12.5h |
| 6. Claude SDK Integration | 8 hours | 20.5h |
| 7. Styling & Polish | 3 hours | 23.5h |
| 8. Chart Integration | 4 hours | 27.5h |
| 9. Testing & Refinement | 3 hours | 30.5h |
| 10. Final Polish | 2 hours | 32.5h |

**Total: ~32.5 hours (4 days)**

---

## Milestones & Checkpoints

### Milestone 1: Text Input Working
**Goal**: Can type and submit text
**Verify**: `cargo run --example simple` works
**Time**: End of Phase 2 (~4.5 hours)

### Milestone 2: UI Layout Complete
**Goal**: Full layout with all areas visible
**Verify**: `cargo run --example full` shows proper layout
**Time**: End of Phase 3 (~7.5 hours)

### Milestone 3: Message Display Working
**Goal**: Messages appear and scroll correctly
**Verify**: Can see conversation history
**Time**: End of Phase 4 (~10.5 hours)

### Milestone 4: Full Chat Flow
**Goal**: Complete conversation cycle (even with mock responses)
**Verify**: User → Loading → Response cycle works
**Time**: End of Phase 5 (~12.5 hours)

### Milestone 5: Claude Integration
**Goal**: Real Claude responses
**Verify**: Actual AI chat works
**Time**: End of Phase 6 (~20.5 hours)

### Milestone 6: Chart Integration
**Goal**: Chat + Chart working together
**Verify**: Integrated app runs
**Time**: End of Phase 8 (~27.5 hours)

### Milestone 7: Production Ready
**Goal**: Polished, tested, documented
**Verify**: All tests pass, examples work, docs complete
**Time**: End of Phase 10 (~32.5 hours)

---

## Risk Mitigation

### Risk 1: bevy_ui_text_input Doesn't Work as Expected
**Likelihood**: Low
**Impact**: High
**Mitigation**: Test in Phase 2 immediately; fallback to bevy_egui if needed
**Time Loss**: 4-6 hours to switch

### Risk 2: Performance Issues with Many Messages
**Likelihood**: Medium
**Impact**: Medium
**Mitigation**: Implement message limit (e.g., 100 messages max) or virtual scrolling
**Time Loss**: 2-4 hours

### Risk 3: Claude SDK Integration Complexity
**Likelihood**: Medium
**Impact**: Medium
**Mitigation**: Already have abstrac-tree implementation to reference
**Time Loss**: 2-3 hours

### Risk 4: Layout Issues on Different Screen Sizes
**Likelihood**: Low
**Impact**: Low
**Mitigation**: Test on multiple window sizes during Phase 3
**Time Loss**: 1-2 hours

---

## Alternative Approaches (If Blocked)

### If bevy_ui_text_input fails:
**Switch to bevy_egui** (adds ~2 hours for learning curve, saves ~4 hours on text input implementation)

### If performance is poor:
**Message virtualization** - Only render visible messages (adds ~6 hours)

### If Bevy UI is too complex:
**Use abstrac-tree Ratatui + egui_ratatui** - Port existing working UI (adds ~8 hours but more proven)

---

## Success Criteria

### Minimum Viable Product (MVP):
- ✅ Can type messages
- ✅ Messages appear in history
- ✅ Scrolling works
- ✅ Claude responds (even if slow)
- ✅ Integrated with chart app

### Full Success:
- ✅ All MVP criteria
- ✅ Rich text formatting (code blocks)
- ✅ Fast performance (60fps)
- ✅ Error handling
- ✅ Loading indicators
- ✅ Configurable styling
- ✅ Clean code and documentation

---

## Development Commands Cheat Sheet

```bash
# Build workspace
cargo build --workspace

# Build only chat-ui
cargo build --package chat-ui

# Run examples
cargo run --package chat-ui --example simple
cargo run --package chat-ui --example messages
cargo run --package chat-ui --example full

# Run integrated app
cargo run --package charts

# Format code
cargo fmt --all

# Check for issues
cargo clippy --all

# Run tests
cargo test --workspace

# Clean build (if needed)
cargo clean
```

---

## Next Steps

1. Review this plan
2. Clarify any questions
3. Execute Phase 0 (workspace setup)
4. Begin Phase 1 (core components)

**Ready to start?** Begin with:
```bash
cd /home/molaco/Documents/bevy-migration
# Update Cargo.toml to create workspace
# Then: cargo new --lib chat-ui
```
