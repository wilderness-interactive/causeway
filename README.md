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

## 49 Tools

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
dedicated_profile = false
restore_session = false
profile = ""
user_data_dir = ""
extensions = []
```

| Field | Description |
|-------|-------------|
| `executable` | Path to your Chromium browser |
| `port` | CDP debugging port (default 9222) |
| `dedicated_profile` | `true` = isolated profile, `false` = your normal browser profile |
| `restore_session` | Reopen tabs from last session on relaunch |
| `profile` | Chromium profile directory name (e.g. `"Profile 1"`) — used with `dedicated_profile = true` |
| `user_data_dir` | Path to the browser's User Data folder — used with `profile` |
| `extensions` | Paths to unpacked extensions to load |

For personal overrides (paths, profiles), create `local_causeway.toml` — same format, gitignored, takes priority.

### Connect to Claude Code

Causeway is an MCP server — Claude Code needs to know where to find it. Create a file called `.mcp.json` in the root of whatever project you want to use Causeway from (not in the Causeway folder itself):

```json
{
  "mcpServers": {
    "causeway": {
      "type": "stdio",
      "command": "cmd",
      "args": ["/c", "C:\\path\\to\\causeway\\target\\debug\\causeway.exe"]
    }
  }
}
```

Replace the path with wherever you cloned and built Causeway. The config files are found automatically relative to the binary.

This works in both **VSCode** (Claude Code extension) and the **CLI** (`claude` in terminal). In VSCode, restart the window (`Ctrl+Shift+P` → "Developer: Reload Window") after creating the file. In the CLI, you can also add it with:

```
claude mcp add causeway -- cmd /c "C:\path\to\causeway\target\debug\causeway.exe"
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

## How It Connects

Chromium browsers only accept automation commands when launched with a special flag (`--remote-debugging-port`). This is a Chromium requirement, not a Causeway one — every CDP tool works this way.

When Causeway's first tool is called, it checks if the browser is already running with that flag. If it is, it connects. If not, it needs to close the browser and reopen it with the flag enabled. This happens automatically.

### Dedicated vs Shared Profile

**Dedicated profile** (`dedicated_profile = true`): Causeway launches a separate browser instance with its own profile. Your normal browsing is completely untouched — you won't even notice it running.

**Shared profile** (`dedicated_profile = false`, the default): Causeway uses your normal browser profile with all your logins and cookies. The trade-off is that if the browser is already open without the debugging flag, Causeway will close it and relaunch it. Your tabs will be restored if `restore_session = true`.

For advanced users, `dedicated_profile = true` is the smoothest experience.

## Troubleshooting

**Browser tools hang or time out**: The browser may have lost its connection. Close the browser and let Causeway relaunch it on the next tool call. Or just try the tool again — Causeway detects dead connections and reconnects automatically.

**"Browser did not become ready"**: The browser failed to start with the debugging flag. This usually means old browser processes are lingering in the background. Open Task Manager, end all instances of your browser (e.g. `brave.exe`, `chrome.exe`, `msedge.exe`), and try again. Causeway does this automatically, but occasionally a process resists.

**Browser closes unexpectedly**: If you're using shared profile mode (the default), this is normal on first connect — Causeway needs to relaunch the browser with the debugging flag. Add `restore_session = true` to your config to keep your tabs. This defaults to false for privacy.

**Cloudflare blocks the page**: Causeway includes stealth mode to bypass bot detection, but previously set cookies from a blocked session may persist. Use the `clear_storage` tool on the affected domain, then try again.

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