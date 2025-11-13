use bevy::prelude::*;
use bevy_tokio_tasks::TokioTasksRuntime;
use claude_agent_sdk::{ClaudeSDKClient, ClaudeAgentOptions, Message as ClaudeMessage, ContentBlock};
use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
use claude_agent_sdk::types::{McpServerConfig, SdkMcpServerMarker, PermissionMode};
use tokio::sync::mpsc;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

use crate::components::{ChatState, ChatMessage};

/// Resource to hold the Claude client channels for async communication
#[derive(Resource)]
pub struct ClaudeClient {
    /// Channel to send user messages to the Claude client task
    sender: mpsc::UnboundedSender<String>,
    /// Channel to receive responses from the Claude client task
    receiver: mpsc::UnboundedReceiver<String>,
}

/// Create an SDK MCP server with chart tools
fn create_chart_tools_mcp_server() -> SdkMcpServer {
    SdkMcpServer::new("chart_tools")
        .version("1.0.0")
        .tool(SdkMcpTool::new(
            "take_chart_screenshot",
            "Take a screenshot of the current chart visualization. Use this when you need to see the actual visual state of the chart.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Optional custom filename for the screenshot (without extension)"
                    }
                }
            }),
            |input| {
                Box::pin(async move {
                    let path = input.get("path")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    // Call BRP endpoint to trigger screenshot via Bevy Remote Protocol
                    // BRP expects JSON-RPC 2.0 format
                    let client = reqwest::Client::new();
                    let brp_request = json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "chart/screenshot",
                        "params": if let Some(p) = &path {
                            json!({ "path": p })
                        } else {
                            json!({})
                        }
                    });

                    let response = client
                        .post("http://127.0.0.1:15702")
                        .json(&brp_request)
                        .send()
                        .await
                        .map_err(|e| std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to call BRP endpoint: {}", e)
                        ))?;

                    let result: serde_json::Value = response
                        .json()
                        .await
                        .map_err(|e| std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to parse response: {}", e)
                        ))?;

                    // BRP returns JSON-RPC format: { "result": { "success": true, "path": "..." } }
                    let filename = result["result"]["path"]
                        .as_str()
                        .ok_or_else(|| std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("No path in BRP response: {:?}", result)
                        ))?;

                    println!("📸 [CHART TOOLS] Screenshot saved: {}", filename);
                    Ok(ToolResult::text(format!(
                        "Screenshot successfully saved to: {}\n\nYou can now analyze this image to see the chart's visual state.",
                        filename
                    )))
                })
            },
        ))
}

/// Initialize the Claude client and spawn the background task
/// This system runs once at startup
pub fn init_claude_client(mut commands: Commands, runtime: ResMut<TokioTasksRuntime>) {

    // Create channels for bidirectional communication
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let (response_tx, response_rx) = mpsc::unbounded_channel::<String>();

    // Spawn async task to manage Claude client using the Tokio runtime
    // This provides a full tokio runtime with IO reactor support for process spawning
    // The task is spawned in the background and runs independently
    runtime.spawn_background_task(|_ctx| async move {

        // Create SDK MCP server for chart tools
        let screenshot_server = create_chart_tools_mcp_server();

        let mut mcp_servers = HashMap::new();
        mcp_servers.insert(
            "chart_tools".to_string(),
            McpServerConfig::Sdk(SdkMcpServerMarker {
                name: "chart_tools".to_string(),
                instance: Arc::new(screenshot_server),
            }),
        );

        // Build options for the Claude client
        let options = ClaudeAgentOptions::builder()
            .system_prompt("You are a helpful chart analysis assistant. You help users understand and analyze financial charts and data. Be concise and clear in your explanations.\n\nYou have access to a tool called 'take_chart_screenshot' that can capture the current visual state of the chart. Use this tool when the user asks about the chart's visual appearance or when you need to see what's currently displayed.")
            .max_turns(50)
            .mcp_servers(mcp_servers)
            .permission_mode(PermissionMode::BypassPermissions)
            .build();

        // Create the Claude SDK client
        let mut client = match ClaudeSDKClient::new(options, None).await {
            Ok(c) => {
                c
            }
            Err(_e) => {
                return;
            }
        };

        // Main message loop - process user messages and stream responses
        while let Some(user_message) = rx.recv().await {

            // Send the user message to Claude
            if let Err(e) = client.send_message(&user_message).await {
                let error_msg = format!("Error: Failed to send message ({})", e);
                if response_tx.send(error_msg).is_err() {
                }
                continue;
            }

            // Collect response text from the message stream
            let mut full_response = String::new();

            // Receive and process messages from Claude
            while let Some(msg_result) = client.next_message().await {
                match msg_result {
                    Ok(ClaudeMessage::Assistant { message, .. }) => {
                        // Extract text content from the assistant message
                        for block in message.content {
                            match block {
                                ContentBlock::Text { text } => {
                                    full_response.push_str(&text);
                                }
                                ContentBlock::Thinking { thinking: _, .. } => {
                                    // Claude's extended thinking - log but don't display
                                }
                                ContentBlock::ToolUse { name, input, .. } => {
                                    // Log tool usage and optionally show to user
                                    println!("🔧 [TOOL USE] {} with input: {:?}", name, input);
                                    if name == "take_chart_screenshot" {
                                        full_response.push_str("\n[Taking screenshot...]\n");
                                    }
                                }
                                ContentBlock::ToolResult { content, .. } => {
                                    // Log tool results
                                    println!("✅ [TOOL RESULT] {:?}", content);
                                }
                            }
                        }
                    }
                    Ok(ClaudeMessage::Result { duration_ms, total_cost_usd, .. }) => {
                        // Conversation turn completed
                        let _ = (duration_ms, total_cost_usd); // Silence unused warnings

                        // Send the accumulated response if we have any
                        if !full_response.is_empty() {
                            if response_tx.send(full_response).is_err() {
                            }
                        }
                        break; // Exit the message receiving loop
                    }
                    Ok(ClaudeMessage::System { subtype: _, .. }) => {
                        // System notification
                    }
                    Ok(ClaudeMessage::StreamEvent { .. }) => {
                        // Streaming event (partial updates) - log if needed
                    }
                    Ok(ClaudeMessage::User { .. }) => {
                        // Echo of user message (log only)
                    }
                    Err(e) => {
                        let error_msg = format!("Error: Failed to receive response ({})", e);
                        if response_tx.send(error_msg).is_err() {
                        }
                        break;
                    }
                }
            }
        }

        // Cleanup
        if let Err(_e) = client.close().await {
        }
    });

    // Insert the resource with channels for the Bevy systems to use
    commands.insert_resource(ClaudeClient {
        sender: tx,
        receiver: response_rx,
    });

}

/// System to send user messages to Claude
/// This runs every frame and checks for new user messages
pub fn send_to_claude(
    mut chat_state: ResMut<ChatState>,
    claude_client: Res<ClaudeClient>,
    mut last_count: Local<usize>,
) {
    let current_count = chat_state.messages.len();

    // Check if there are new messages and we're waiting for a response
    if current_count > *last_count {
        if let Some(last_msg) = chat_state.messages.last() {
            // Only send if it's a user message and we're in waiting state
            if last_msg.is_user && chat_state.is_waiting {
                // Build the message to send, including context if available
                let message_to_send = if let Some(context) = &chat_state.chart_context {
                    format!("[Context: {}]\n\nUser: {}", context, last_msg.content)
                } else {
                    last_msg.content.clone()
                };

                if let Err(_e) = claude_client.sender.send(message_to_send) {
                }

                // Clear the chart context after sending so it doesn't get reused
                chat_state.chart_context = None;
            }
        }
        *last_count = current_count;
    }
}

/// System to receive responses from Claude
/// This runs every frame and checks for new responses
pub fn receive_from_claude(
    mut chat_state: ResMut<ChatState>,
    mut claude_client: ResMut<ClaudeClient>,
) {
    // Process all available responses (non-blocking)
    while let Ok(response) = claude_client.receiver.try_recv() {

        // Add the assistant's response to the chat history
        chat_state.messages.push(ChatMessage {
            content: response,
            is_user: false,
            timestamp: std::time::SystemTime::now(),
        });

        // Clear the waiting state
        chat_state.is_waiting = false;
    }
}
