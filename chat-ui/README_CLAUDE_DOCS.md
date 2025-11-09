# Claude Chat SDK Documentation

This directory contains comprehensive documentation about the `claude-agent-sdk-rust` implementation, extracted from the `/home/molaco/Documents/japanese` codebase.

## Documentation Files

### 1. **EXPLORATION_SUMMARY.txt** (START HERE)
- High-level overview of the exploration
- Executive summary of findings
- Key insights and gotchas
- Integration recommendations
- Next steps for implementation

**Read this first** (5-10 minutes)

### 2. **CLAUDE_INTEGRATION_SUMMARY.md** (QUICK REFERENCE)
- Quick reference for chat-ui integration
- Core architecture overview
- Two main APIs (Simple Query vs Interactive Client)
- Background tasks explanation
- Communication flow diagrams
- Configuration examples
- Integration checklist

**Read this second** (10-15 minutes) to understand the architecture

### 3. **CLAUDE_SDK_REFERENCE.md** (COMPLETE REFERENCE)
- Comprehensive implementation details
- 14 major sections covering all systems
- File structure and organization
- Detailed API documentation
- Code patterns and examples
- Complete error handling approach
- Security implementation details
- Example usage patterns for all features

**Read this for deep dives** (30+ minutes) on specific systems

### 4. **CLAUDE_SDK_FILES.md** (CODE LOCATIONS)
- File-by-file breakdown of SDK
- Key types and methods per file
- Code location references with line numbers
- Dependencies and their usage
- Essential reading order
- Quick code locations table

**Use this as a reference** when exploring the actual code

## Quick Start

1. **First 10 minutes**: Read `EXPLORATION_SUMMARY.txt`
   - Get the big picture
   - Understand key concepts
   - See recommended approach

2. **Next 15 minutes**: Read `CLAUDE_INTEGRATION_SUMMARY.md`
   - Learn the two APIs
   - See architecture diagrams
   - Review integration patterns

3. **As needed**: Refer to `CLAUDE_SDK_REFERENCE.md`
   - Look up specific systems (Hooks, Permissions, MCP)
   - Study implementation patterns
   - Review error handling

4. **When coding**: Use `CLAUDE_SDK_FILES.md`
   - Find file locations
   - Look up method signatures
   - Understand dependencies

## Key Concepts

### Two Main APIs

**Simple Query API** (`query()`)
- Stateless, one-shot interactions
- Returns stream of messages
- No connection management needed
- Best for: CLI tools, batch processing

**Interactive Client API** (`ClaudeSDKClient`)
- Stateful conversations
- Bidirectional communication
- Lock-free concurrent operations
- Best for: Chat UI, REPL-like interfaces

### Background Tasks (Automatic)

The SDK spawns 6 background tasks automatically:
1. **Message Reader** - Reads CLI output
2. **Control Writer** - Writes commands
3. **Hook Handler** - Processes hook events (if configured)
4. **Hook Callback Handler** - Routes hook callbacks (if configured)
5. **Permission Handler** - Checks permissions (if configured)
6. **MCP Message Handler** - Processes MCP requests (if configured)

**Key insight**: No manual task management needed - all handled internally!

### Lock-Free Concurrency

The critical innovation is the **lock-free read/write pattern**:
1. Reader task acquires lock ONCE to get receiver channel
2. Releases immediately
3. Processes messages from channel (no lock needed)
4. Writer task locks only during actual write
5. Result: **NO CONTENTION** between send and receive

This enables safe concurrent send/receive without blocking.

## For chat-ui Integration

### Recommended Approach

```rust
// 1. Create options
let options = ClaudeAgentOptions::builder()
    .system_prompt("You are helpful")
    .max_turns(10)
    .build();

// 2. Create client
let mut client = ClaudeSDKClient::new(options, None).await?;

// 3. Send message
client.send_message("Hello!").await?;

// 4. Receive messages
while let Some(msg) = client.next_message().await {
    match msg? {
        Message::Assistant { message, .. } => {
            // Display response
        }
        Message::Result { duration_ms, total_cost_usd, .. } => {
            // Conversation ended
            break;
        }
        _ => {}
    }
}

// 5. Cleanup
client.close().await?;
```

### Message Types to Handle

| Type | Purpose | Action |
|------|---------|--------|
| `Assistant { message, ... }` | Claude's response | Extract and display text |
| `Result { duration_ms, ... }` | Session metrics | End conversation |
| `System { ... }` | Notifications | Log or ignore |
| `StreamEvent { ... }` | Partial updates | Log or ignore |
| `User { ... }` | User input (echoed) | Log or ignore |

**Note**: Control messages are filtered out automatically.

### Optional Features

Add as needed:

**Hooks** - Monitor tool usage
```rust
.hooks(hooks_config)
```

**Permissions** - Control tool access
```rust
.can_use_tool(permission_callback)
```

**MCP Servers** - Add custom tools
```rust
.mcp_servers(servers_config)
```

## Important Gotchas

1. **Control messages don't appear**
   - `control_response` and `control_request` are filtered out
   - Only application messages reach your code

2. **Hook callback IDs**
   - Generated as `hook_0`, `hook_1`, etc.
   - Must match between initialization and invocation

3. **Streaming mode is automatic**
   - Enabled for `ClaudeSDKClient`
   - Enabled if hooks/permissions/MCP configured
   - Otherwise uses print mode

4. **Permission defaults**
   - No allowed_tools = all allowed
   - With allowed_tools = only those allowed
   - Disallowed_tools checked first (deny wins)

5. **Reader lock pattern**
   - Lock held only to get receiver
   - Released immediately
   - Safe for concurrent operations

## Original Source Code

All SDK source code is located at:
```
/home/molaco/Documents/japanese/claude-agent-sdk-rust/
```

### Key files to study
- `src/client/mod.rs` - Main client implementation
- `src/types.rs` - Type definitions
- `src/transport/subprocess.rs` - CLI communication
- `examples/interactive_client.rs` - Interactive example

See `CLAUDE_SDK_FILES.md` for complete file listing and line numbers.

## Dependencies

Core dependencies (from SDK):
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization
- `async-trait` - Async trait support
- `thiserror` - Error types
- `futures` - Streaming
- `which` - Binary discovery

## Next Steps

1. **Understand the architecture**
   - Read EXPLORATION_SUMMARY.txt
   - Read CLAUDE_INTEGRATION_SUMMARY.md

2. **Study implementation details**
   - Read CLAUDE_SDK_REFERENCE.md
   - Focus on ClaudeSDKClient section

3. **Plan integration**
   - Review integration checklist
   - Design message handling
   - Plan error handling

4. **Implement**
   - Start with basic interactive client
   - Add UI integration
   - Add optional features (hooks/permissions/MCP)

5. **Reference as needed**
   - Use CLAUDE_SDK_FILES.md for code locations
   - Use CLAUDE_SDK_REFERENCE.md for specific systems

## Document Statistics

| Document | Size | Lines | Purpose |
|----------|------|-------|---------|
| EXPLORATION_SUMMARY.txt | 8 KB | 200+ | Overview and next steps |
| CLAUDE_INTEGRATION_SUMMARY.md | 15 KB | 515 | Quick reference |
| CLAUDE_SDK_REFERENCE.md | 31 KB | 1089 | Complete reference |
| CLAUDE_SDK_FILES.md | 12 KB | 489 | File locations and code refs |

**Total**: 66 KB of comprehensive documentation

## Key Takeaways

1. **Production-quality SDK** - Well-architected, security-hardened
2. **Two clear APIs** - Choose based on your use case
3. **Automatic concurrency** - Background tasks handled internally
4. **Lock-free design** - No contention between read and write
5. **Extensible** - Hooks, permissions, and custom MCP tools
6. **Comprehensive errors** - Rich context for debugging
7. **Type-safe** - Builder pattern with sensible defaults

The SDK is ready to be integrated into chat-ui with minimal changes!

---

**Documentation generated**: November 9, 2025
**Source**: `/home/molaco/Documents/japanese/claude-agent-sdk-rust/`
**Exploration thoroughness**: Very Thorough
