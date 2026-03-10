# Causeway

Sovereign browser bridge. AI meets browser. Data flows between.

Causeway is a Rust binary that drives Chromium via Chrome DevTools Protocol. It speaks MCP over stdio to your AI, and CDP over WebSocket to your browser. One binary. Two protocols. Complete browser control.

**[wildernessinteractive.com/causeway](https://wildernessinteractive.com/causeway)**

## Architecture

```
Claude Code <--stdio/MCP--> causeway.exe <--WebSocket/CDP--> Browser
```

- **MCP layer**: rmcp over stdio — structured tool calls, not natural language
- **CDP layer**: Raw JSON-RPC over WebSocket — real browser, real rendering
- **No extensions, no Node, no Python** — one Rust binary

## 47 Tools

Navigation, tab management, clicking, typing, form filling, screenshots, page reading, accessibility snapshots, JavaScript evaluation, cookie control, network monitoring, file downloads, PDF saving, device emulation, and more.

## Stealth Mode

Bot detection systems like Cloudflare Bot Fight Mode look for CDP fingerprints. Causeway erases them before page scripts execute. Your real browser. Your real session. No automation footprint.

## Setup

### Build

```
cargo build
```

### Configure

Create `causeway.toml` in the project root:

```toml
[browser]
executable = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"
port = 9222
```

### Connect to Claude Code

Add to your project's `.mcp.json`:

```json
{
  "mcpServers": {
    "causeway": {
      "type": "stdio",
      "command": "path/to/causeway.exe"
    }
  }
}
```

Causeway finds its config automatically: working directory, then source root, then next to the binary.

## Design

No objects. No state. Just data.

CDP commands are pure functions returning `(&str, Value)` pairs. The bridge function sends them over WebSocket. That's it.

```rust
pub fn navigate(url: &str) -> (&'static str, Value) {
    ("Page.navigate", json!({ "url": url }))
}
```

## License

Wilderness Interactive Open License

Permission is hereby granted, free of charge, to use, copy, modify, and distribute this software for any purpose, including commercial use.

This software may NOT be:
- Sold as a standalone product
- Sold access to as a hosted service

Use for building software, building websites, automating workflows, and integrating with other tools (including commercial work) is explicitly permitted and encouraged. This software is designed to be moddable, so modifications are explicitly permitted and encouraged. Software and systems built using this tool can be sold freely.

The purpose of this license is to prevent reselling the software itself.

---

Built by [Wilderness Interactive](https://wildernessinteractive.com).