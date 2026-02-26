# Causeway

Sovereign browser bridge — drives Chromium via Chrome DevTools Protocol.

## Architecture

```
Claude Code <--stdio/MCP--> causeway.exe <--WebSocket/CDP--> Brave
```

- MCP layer: rmcp 0.15 over stdio
- CDP layer: Raw JSON-RPC over WebSocket
- No extensions, no Node, no Python

## Data Flow

CDP commands are pure data: `fn navigate(url) -> (&str, Value)`. The CDP client routes by id (responses) or method (events). No OOP, no central state.

## Config

`causeway.toml` — browser executable path, CDP port, profile preference.

## Building

```
cargo build
```

## Using

Add to any project's `.mcp.json`:
```json
{
  "mcpServers": {
    "causeway": {
      "type": "stdio",
      "command": "cmd",
      "args": ["/c", "E:\\Claude Code\\webmcp\\target\\debug\\causeway.exe"],
      "cwd": "E:\\Claude Code\\webmcp"
    }
  }
}
```
