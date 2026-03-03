use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};

use crate::cdp::{self, LiveConnection};
use crate::commands;

// -- Tool parameter structs --

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct NavigateParams {
    #[schemars(description = "The URL to navigate to")]
    pub url: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct EvaluateJsParams {
    #[schemars(description = "JavaScript expression to evaluate in the page context")]
    pub expression: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ClickParams {
    #[schemars(description = "CSS selector of the element to click")]
    pub selector: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TypeTextParams {
    #[schemars(description = "CSS selector of the element to type into")]
    pub selector: String,
    #[schemars(description = "The text to type")]
    pub text: String,
    #[schemars(description = "Clear the field before typing (select all + delete). Default: false")]
    pub clear: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WaitForParams {
    #[schemars(description = "CSS selector to wait for")]
    pub selector: String,
    #[schemars(description = "Maximum time to wait in milliseconds (default: 5000)")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ScrollParams {
    #[schemars(description = "Pixels to scroll horizontally (positive = right)")]
    pub x: Option<f64>,
    #[schemars(description = "Pixels to scroll vertically (positive = down)")]
    pub y: Option<f64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SelectOptionParams {
    #[schemars(description = "CSS selector of the <select> element")]
    pub selector: String,
    #[schemars(description = "The value attribute of the option to select")]
    pub value: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SwitchTabParams {
    #[schemars(description = "The target ID of the tab to switch to (from list_tabs)")]
    pub target_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct NewTabParams {
    #[schemars(description = "URL to open in the new tab (default: about:blank)")]
    pub url: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CloseTabParams {
    #[schemars(description = "The target ID of the tab to close (from list_tabs)")]
    pub target_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct InspectParams {
    #[schemars(description = "CSS selector to inspect (default: body)")]
    pub selector: Option<String>,
    #[schemars(description = "Maximum depth to traverse (default: 4)")]
    pub max_depth: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct QueryElementsParams {
    #[schemars(description = "CSS selector to find matching elements")]
    pub selector: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ClickTextParams {
    #[schemars(description = "The text to search for in element content (case-insensitive substring match)")]
    pub text: String,
    #[schemars(description = "HTML tag to limit search to (e.g. \"button\", \"a\"). Default: \"*\" (all elements)")]
    pub tag: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct HoverParams {
    #[schemars(description = "CSS selector of the element to hover over")]
    pub selector: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PressKeyParams {
    #[schemars(description = "Key to press (e.g. \"Enter\", \"Tab\", \"Escape\", \"ArrowDown\", \"Backspace\", \"Space\")")]
    pub key: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetAttributeParams {
    #[schemars(description = "CSS selector of the element")]
    pub selector: String,
    #[schemars(description = "Attribute name to read (e.g. \"href\", \"src\", \"data-id\", \"value\")")]
    pub attribute: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReadTextParams {
    #[schemars(description = "CSS selector to read text from")]
    pub selector: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FillFormParams {
    #[schemars(description = "CSS selector of the form or container element")]
    pub selector: String,
    #[schemars(description = "JSON object mapping field names/selectors to values, e.g. {\"#email\": \"test@example.com\", \"#name\": \"John\"}")]
    pub fields: std::collections::HashMap<String, String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WaitForNavigationParams {
    #[schemars(description = "Maximum time to wait in milliseconds (default: 10000)")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetCookiesParams {
    #[schemars(description = "Optional URL filter — only return cookies for this domain. If omitted, returns cookies for the current page.")]
    pub url: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WaitForTextParams {
    #[schemars(description = "Text to wait for (case-insensitive substring match)")]
    pub text: String,
    #[schemars(description = "CSS selector of the container element to search within (default: body)")]
    pub selector: Option<String>,
    #[schemars(description = "Maximum time to wait in milliseconds (default: 5000)")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetCookieParams {
    #[schemars(description = "Cookie name")]
    pub name: String,
    #[schemars(description = "Cookie value")]
    pub value: String,
    #[schemars(description = "URL to associate the cookie with (used to infer domain/path if not provided)")]
    pub url: Option<String>,
    #[schemars(description = "Cookie domain (e.g. \".example.com\")")]
    pub domain: Option<String>,
    #[schemars(description = "Cookie path (default: \"/\")")]
    pub path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UploadFileParams {
    #[schemars(description = "CSS selector of the <input type=\"file\"> element")]
    pub selector: String,
    #[schemars(description = "Absolute path to the file to upload")]
    pub file_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct HandleDialogParams {
    #[schemars(description = "true to accept (OK/Yes), false to dismiss (Cancel/No)")]
    pub accept: bool,
    #[schemars(description = "Text to enter for prompt dialogs")]
    pub prompt_text: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct KeyboardChordParams {
    #[schemars(description = "Key chord to press, e.g. \"Ctrl+A\", \"Ctrl+Shift+T\", \"Alt+F4\". Modifier names: Ctrl, Alt, Shift, Meta.")]
    pub chord: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DoubleClickParams {
    #[schemars(description = "CSS selector of the element to double-click")]
    pub selector: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DragParams {
    #[schemars(description = "CSS selector of the element to drag from (preferred over from_x/from_y)")]
    pub from_selector: Option<String>,
    #[schemars(description = "CSS selector of the element to drag to (preferred over to_x/to_y)")]
    pub to_selector: Option<String>,
    #[schemars(description = "X coordinate to start drag (used when from_selector not provided)")]
    pub from_x: Option<f64>,
    #[schemars(description = "Y coordinate to start drag (used when from_selector not provided)")]
    pub from_y: Option<f64>,
    #[schemars(description = "X coordinate to drag to (used when to_selector not provided)")]
    pub to_x: Option<f64>,
    #[schemars(description = "Y coordinate to drag to (used when to_selector not provided)")]
    pub to_y: Option<f64>,
    #[schemars(description = "Number of intermediate steps for smooth drag (default: 10)")]
    pub steps: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetViewportParams {
    #[schemars(description = "Viewport width in pixels")]
    pub width: u32,
    #[schemars(description = "Viewport height in pixels")]
    pub height: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetConsoleMessagesParams {
    #[schemars(description = "Filter by level: \"log\", \"info\", \"warn\", \"error\", \"debug\". Omit for all.")]
    pub level: Option<String>,
    #[schemars(description = "Clear the console log buffer after reading (default: false)")]
    pub clear: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListNetworkRequestsParams {
    #[schemars(description = "Filter by URL substring (case-insensitive). Omit for all.")]
    pub url_filter: Option<String>,
    #[schemars(description = "Clear the network log buffer after reading (default: false)")]
    pub clear: Option<bool>,
}

// -- Shared JS helpers --

/// Build JS that finds the first visible, in-viewport element matching a selector.
/// Returns JS that resolves to `{ x, y }` or `null`.
fn js_find_visible_element(selector: &str) -> String {
    format!(
        r#"(async () => {{
            const els = document.querySelectorAll({sel});
            if (!els.length) return null;
            const vw = window.innerWidth;
            const vh = window.innerHeight;
            for (const el of els) {{
                const r = el.getBoundingClientRect();
                if (r.width === 0 || r.height === 0) continue;
                el.scrollIntoView({{ block: 'center', behavior: 'instant' }});
                await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
                const rect = el.getBoundingClientRect();
                const cx = rect.x + rect.width / 2;
                const cy = rect.y + rect.height / 2;
                if (cx >= 0 && cy >= 0 && cx <= vw && cy <= vh) {{
                    return {{ x: cx, y: cy }};
                }}
            }}
            return null;
        }})()"#,
        sel = serde_json::to_string(selector).unwrap()
    )
}

/// Build JS that finds the first visible element matching selector and focuses it.
/// Optionally selects all text (for clearing). Returns JS that resolves to `true` or `false`.
fn js_focus_visible_element(selector: &str, should_clear: bool) -> String {
    format!(
        r#"(async () => {{
            const els = document.querySelectorAll({sel});
            for (const el of els) {{
                const r = el.getBoundingClientRect();
                if (r.width === 0 || r.height === 0) continue;
                el.scrollIntoView({{ block: 'center', behavior: 'instant' }});
                await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
                el.focus();
                if ({clear}) el.select();
                return true;
            }}
            return false;
        }})()"#,
        sel = serde_json::to_string(selector).unwrap(),
        clear = should_clear
    )
}

// -- Event buffer data --

#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    pub level: String,
    pub text: String,
    #[allow(dead_code)] // stored for future sorting/filtering
    pub timestamp: f64,
}

#[derive(Debug, Clone)]
pub struct NetworkEntry {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub status: Option<u16>,
    #[allow(dead_code)] // stored for future sorting/filtering
    pub timestamp: f64,
}

// -- Chord parsing --

/// Parse "Ctrl+Shift+A" → (modifiers_bitmask, key_string).
/// Modifiers: Alt=1, Ctrl=2, Meta=4, Shift=8.
fn parse_chord(chord: &str) -> (u32, String) {
    let parts: Vec<&str> = chord.split('+').collect();
    let mut modifiers = 0u32;
    let mut key_part = String::new();

    for part in &parts {
        match part.trim().to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= 2,
            "alt" => modifiers |= 1,
            "meta" | "cmd" | "super" | "win" => modifiers |= 4,
            "shift" => modifiers |= 8,
            _ => key_part = part.trim().to_owned(),
        }
    }

    // Single letter: lowercase normally, uppercase if Shift
    let key = if key_part.len() == 1 && key_part.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        if modifiers & 8 != 0 { key_part.to_uppercase() } else { key_part.to_lowercase() }
    } else {
        // Named key like "Enter", "F5" — capitalize first letter
        let mut chars = key_part.chars();
        match chars.next() {
            None => key_part,
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        }
    };

    (modifiers, key)
}

// -- Accessibility tree rendering --

fn render_ax_tree(nodes: &[serde_json::Value]) -> String {
    use std::collections::HashMap;

    let mut by_id: HashMap<&str, &serde_json::Value> = HashMap::new();
    for node in nodes {
        if let Some(id) = node.get("nodeId").and_then(|v| v.as_str()) {
            by_id.insert(id, node);
        }
    }

    // Root = no parentId, or parent not in the map
    let mut root_ids: Vec<&str> = Vec::new();
    for node in nodes {
        let id = match node.get("nodeId").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => continue,
        };
        let has_parent_in_tree = node
            .get("parentId")
            .and_then(|v| v.as_str())
            .map(|p| by_id.contains_key(p))
            .unwrap_or(false);
        if !has_parent_in_tree {
            root_ids.push(id);
        }
    }

    let mut output = String::new();
    for root_id in &root_ids {
        ax_walk(root_id, &by_id, 0, &mut output);
    }
    output
}

fn ax_walk(
    node_id: &str,
    by_id: &std::collections::HashMap<&str, &serde_json::Value>,
    depth: usize,
    output: &mut String,
) {
    let node = match by_id.get(node_id) {
        Some(n) => n,
        None => return,
    };

    let role = node
        .get("role")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let ignored = node.get("ignored").and_then(|v| v.as_bool()).unwrap_or(false);
    let skip = ignored || role == "none" || role == "ignored";

    if !skip {
        let name = node
            .get("name")
            .and_then(|n| n.get("value"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        let indent = "  ".repeat(depth);
        if let Some(n) = name {
            output.push_str(&format!("{indent}[{role}] \"{n}\"\n"));
        } else {
            output.push_str(&format!("{indent}[{role}]\n"));
        }
    }

    let next_depth = if skip { depth } else { depth + 1 };

    if let Some(children) = node.get("childIds").and_then(|v| v.as_array()) {
        for child_id in children {
            if let Some(id) = child_id.as_str() {
                ax_walk(id, by_id, next_depth, output);
            }
        }
    }
}

// -- MCP Server --

#[derive(Debug, Clone)]
pub struct CausewayServer {
    live: Arc<LiveConnection>,
    port: u16,
    /// The target ID of the tab we consider "ours". try_reconnect returns here.
    sticky_target: Arc<tokio::sync::Mutex<Option<String>>>,
    console_log: Arc<tokio::sync::Mutex<Vec<ConsoleEntry>>>,
    network_log: Arc<tokio::sync::Mutex<Vec<NetworkEntry>>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CausewayServer {
    pub fn new(live: Arc<LiveConnection>, port: u16) -> Self {
        Self {
            live,
            port,
            sticky_target: Arc::new(tokio::sync::Mutex::new(None)),
            console_log: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            network_log: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Navigate the browser to a URL. Returns the page title after loading.")]
    async fn navigate(
        &self,
        Parameters(NavigateParams { url }): Parameters<NavigateParams>,
    ) -> Result<CallToolResult, McpError> {
        // Page.navigate returns after the navigation commits (new document is ready).
        self.execute_reconnect(commands::navigate(&url))
            .await
            .map_err(|e| McpError::internal_error(format!("Navigate failed: {e}"), None))?;

        // Wait for the page to fully load (readyState = 'complete'). 8s cap.
        let _ = self.execute_reconnect(commands::evaluate(
            "new Promise(resolve => {
                if (document.readyState === 'complete') { resolve(); return; }
                window.addEventListener('load', () => resolve(), { once: true });
                setTimeout(resolve, 8000);
            })",
        ))
        .await;

        let title_result = self.execute_reconnect(commands::evaluate("document.title"))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to get title: {e}"), None))?;

        let title = title_result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)");

        let url_result = self.execute_reconnect(commands::evaluate("window.location.href"))
            .await
            .ok();

        let current_url = url_result
            .as_ref()
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or(&url);

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Navigated to: {current_url}\nTitle: {title}"
        ))]))
    }

    #[tool(description = "Take a screenshot of the current page. Returns the image as base64 PNG.")]
    async fn screenshot(&self) -> Result<CallToolResult, McpError> {
        let result = self.execute_reconnect(commands::screenshot(None, "png"))
            .await
            .map_err(|e| McpError::internal_error(format!("Screenshot failed: {e}"), None))?;

        let data = result
            .get("data")
            .and_then(|d| d.as_str())
            .ok_or_else(|| McpError::internal_error("No screenshot data returned".to_owned(), None))?;

        Ok(CallToolResult::success(vec![Content::image(
            data.to_owned(),
            "image/png",
        )]))
    }

    #[tool(description = "Read the text content of the current page. Returns the visible text.")]
    async fn read_page(&self) -> Result<CallToolResult, McpError> {
        let result = self.execute_reconnect(commands::evaluate("document.body.innerText"))
            .await
            .map_err(|e| McpError::internal_error(format!("Read page failed: {e}"), None))?;

        let text = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("(empty page)");

        // Truncate if extremely long to avoid overwhelming context
        let truncated = if text.len() > 10000 {
            format!("{}...\n\n[Truncated — {} total characters]", &text[..10000], text.len())
        } else {
            text.to_owned()
        };

        Ok(CallToolResult::success(vec![Content::text(truncated)]))
    }

    #[tool(description = "Read text content from a specific element by CSS selector. More focused than read_page — avoids overwhelming output on complex pages.")]
    async fn read_text(
        &self,
        Parameters(ReadTextParams { selector }): Parameters<ReadTextParams>,
    ) -> Result<CallToolResult, McpError> {
        let js = format!(
            r#"(() => {{
                const el = document.querySelector({sel});
                if (!el) return null;
                return el.innerText.trim();
            }})()"#,
            sel = serde_json::to_string(&selector).unwrap()
        );

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Read text failed: {e}"), None))?;

        let value = result
            .get("result")
            .and_then(|r| r.get("value"));

        match value {
            Some(v) if !v.is_null() => {
                let text = v.as_str().unwrap_or("(non-text content)");
                let truncated = if text.len() > 10000 {
                    format!("{}...\n\n[Truncated — {} total characters]", &text[..10000], text.len())
                } else {
                    text.to_owned()
                };
                Ok(CallToolResult::success(vec![Content::text(truncated)]))
            }
            _ => Err(McpError::invalid_params(
                format!("Element not found: {selector}"),
                None,
            )),
        }
    }

    #[tool(description = "Get browser cookies, optionally filtered by URL. Uses CDP Network.getCookies for full cookie details including httpOnly and secure cookies not visible to JavaScript.")]
    async fn get_cookies(
        &self,
        Parameters(GetCookiesParams { url }): Parameters<GetCookiesParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = match &url {
            Some(u) => serde_json::json!({ "urls": [u] }),
            None => serde_json::json!({}),
        };

        let result = self.exec_with_reconnect("Network.getCookies", params)
            .await
            .map_err(|e| McpError::internal_error(format!("Get cookies failed: {e}"), None))?;

        let cookies = result
            .get("cookies")
            .and_then(|c| c.as_array());

        match cookies {
            Some(arr) => {
                let summary: Vec<String> = arr.iter().map(|c| {
                    let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let domain = c.get("domain").and_then(|v| v.as_str()).unwrap_or("?");
                    let value_preview = c.get("value")
                        .and_then(|v| v.as_str())
                        .map(|v| if v.len() > 40 { format!("{}...", &v[..40]) } else { v.to_owned() })
                        .unwrap_or_default();
                    let secure = c.get("secure").and_then(|v| v.as_bool()).unwrap_or(false);
                    let http_only = c.get("httpOnly").and_then(|v| v.as_bool()).unwrap_or(false);
                    let flags = format!("{}{}",
                        if secure { "Secure " } else { "" },
                        if http_only { "HttpOnly" } else { "" }
                    );
                    format!("{name} ({domain}) = {value_preview} [{flags}]")
                }).collect();

                Ok(CallToolResult::success(vec![Content::text(format!(
                    "{} cookies:\n{}",
                    arr.len(),
                    summary.join("\n")
                ))]))
            }
            None => Ok(CallToolResult::success(vec![Content::text(
                "No cookies found".to_owned()
            )])),
        }
    }

    #[tool(description = "Set a browser cookie. Use url to infer domain/path, or provide domain/path explicitly.")]
    async fn set_cookie(
        &self,
        Parameters(SetCookieParams { name, value, url, domain, path }): Parameters<SetCookieParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.execute_reconnect(commands::set_cookie(
            &name,
            &value,
            url.as_deref(),
            domain.as_deref(),
            path.as_deref(),
        ))
        .await
        .map_err(|e| McpError::internal_error(format!("Set cookie failed: {e}"), None))?;

        let success = result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if success {
            Ok(CallToolResult::success(vec![Content::text(format!("Set cookie: {name}"))]))
        } else {
            Err(McpError::internal_error(
                format!("Cookie '{name}' was not set — check domain/url is valid for the current page"),
                None,
            ))
        }
    }

    #[tool(description = "Wait until specific text appears on the page. Polls the container element every 200ms. Case-insensitive substring match.")]
    async fn wait_for_text(
        &self,
        Parameters(WaitForTextParams { text, selector, timeout_ms }): Parameters<WaitForTextParams>,
    ) -> Result<CallToolResult, McpError> {
        let timeout = timeout_ms.unwrap_or(5000);
        let interval = 200u64;
        let max_attempts = timeout / interval;
        let container = selector.as_deref().unwrap_or("body");
        let needle = text.to_lowercase();

        for _ in 0..max_attempts {
            let js = format!(
                r#"(() => {{
                    const el = document.querySelector({sel});
                    return el ? el.innerText.toLowerCase().includes({needle}) : false;
                }})()"#,
                sel = serde_json::to_string(container).unwrap(),
                needle = serde_json::to_string(&needle).unwrap(),
            );

            let result = self.execute_reconnect(commands::evaluate(&js))
                .await
                .map_err(|e| McpError::internal_error(format!("Text check failed: {e}"), None))?;

            let found = result
                .get("result")
                .and_then(|r| r.get("value"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if found {
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Text found: \"{text}\""
                ))]));
            }

            tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
        }

        Err(McpError::internal_error(
            format!("Text \"{text}\" not found within {timeout}ms"),
            None,
        ))
    }

    #[tool(description = "Set files on a <input type=\"file\"> element via CDP — bypasses the OS file picker entirely, no dialog opens. Provide the absolute path to the file.")]
    async fn upload_file(
        &self,
        Parameters(UploadFileParams { selector, file_path }): Parameters<UploadFileParams>,
    ) -> Result<CallToolResult, McpError> {
        // Verify the file exists before attempting to set it.
        let path = std::path::Path::new(&file_path);
        if !path.exists() {
            return Err(McpError::invalid_params(
                format!("File not found: {file_path}"),
                None,
            ));
        }

        // Get a remote object reference to the input element (not returnByValue — we need objectId).
        let js = format!(
            "document.querySelector({sel})",
            sel = serde_json::to_string(&selector).unwrap()
        );
        let ref_result = self.execute_reconnect(commands::evaluate_ref(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to find element: {e}"), None))?;

        let object_id = ref_result
            .get("result")
            .and_then(|r| r.get("objectId"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params(
                format!("Element not found: {selector}"),
                None,
            ))?
            .to_owned();

        // Set the file directly via objectId — no OS picker, no dialog, completely silent.
        self.execute_reconnect(commands::set_file_input_files(&object_id, &[file_path.clone()]))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to set file: {e}"), None))?;

        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&file_path);

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Uploaded '{filename}' to '{selector}'"
        ))]))
    }

    #[tool(description = "Execute JavaScript in the page context and return the result.")]
    async fn evaluate_js(
        &self,
        Parameters(EvaluateJsParams { expression }): Parameters<EvaluateJsParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.execute_reconnect(commands::evaluate(&expression))
            .await
            .map_err(|e| McpError::internal_error(format!("JS evaluation failed: {e}"), None))?;

        // Check for exceptions. On SPA navigation teardown, retry once after a short delay.
        if let Some(exception) = result.get("exceptionDetails") {
            let msg = exception
                .get("exception")
                .and_then(|e| e.get("description"))
                .and_then(|d| d.as_str())
                .unwrap_or("Unknown JS error");

            if msg.contains("global scope") || msg.contains("Cannot read properties of undefined") {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                let retry = self.execute_reconnect(commands::evaluate(&expression))
                    .await
                    .map_err(|e| McpError::internal_error(format!("JS evaluation failed: {e}"), None))?;
                if retry.get("exceptionDetails").is_none() {
                    let value = retry
                        .get("result")
                        .and_then(|r| r.get("value"))
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let output = if value.is_string() {
                        value.as_str().unwrap().to_owned()
                    } else {
                        serde_json::to_string_pretty(&value).unwrap_or_else(|_| format!("{value:?}"))
                    };
                    return Ok(CallToolResult::success(vec![Content::text(output)]));
                }
            }

            return Ok(CallToolResult::success(vec![Content::text(format!(
                "JS Error: {msg}"
            ))]));
        }

        let value = result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let output = if value.is_string() {
            value.as_str().unwrap().to_owned()
        } else {
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| format!("{value:?}"))
        };

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Click an element on the page by CSS selector.")]
    async fn click(
        &self,
        Parameters(ClickParams { selector }): Parameters<ClickParams>,
    ) -> Result<CallToolResult, McpError> {
        let js = js_find_visible_element(&selector);

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to find element: {e}"), None))?;

        let coords = result
            .get("result")
            .and_then(|r| r.get("value"))
            .ok_or_else(|| {
                McpError::invalid_params(format!("No visible, in-viewport element found for: {selector}"), None)
            })?;

        if coords.is_null() {
            return Err(McpError::invalid_params(
                format!("No visible, in-viewport element found for: {selector}"),
                None,
            ));
        }

        let x = coords.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = coords.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);

        self.execute_seq_reconnect(commands::click(x, y))
            .await
            .map_err(|e| McpError::internal_error(format!("Click failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Clicked '{selector}' at ({x:.0}, {y:.0})"
        ))]))
    }

    #[tool(description = "Click an element by its visible text content. More reliable than CSS selectors on dynamic UIs. Finds the first visible, in-viewport element whose text contains the search string.")]
    async fn click_text(
        &self,
        Parameters(ClickTextParams { text, tag }): Parameters<ClickTextParams>,
    ) -> Result<CallToolResult, McpError> {
        let tag_filter = tag.as_deref().unwrap_or("*");

        let js = format!(
            r#"(async () => {{
                const searchText = {text}.toLowerCase();
                const els = document.querySelectorAll({tag});
                const vw = window.innerWidth;
                const vh = window.innerHeight;
                for (const el of els) {{
                    const elText = el.textContent.trim().toLowerCase();
                    if (!elText.includes(searchText)) continue;
                    const r = el.getBoundingClientRect();
                    if (r.width === 0 || r.height === 0) continue;
                    el.scrollIntoView({{ block: 'center', behavior: 'instant' }});
                    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
                    const rect = el.getBoundingClientRect();
                    const cx = rect.x + rect.width / 2;
                    const cy = rect.y + rect.height / 2;
                    if (cx >= 0 && cy >= 0 && cx <= vw && cy <= vh) {{
                        return {{ x: cx, y: cy, matched: el.textContent.trim().substring(0, 80) }};
                    }}
                }}
                return null;
            }})()"#,
            text = serde_json::to_string(&text).unwrap(),
            tag = serde_json::to_string(tag_filter).unwrap()
        );

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to find element: {e}"), None))?;

        let coords = result
            .get("result")
            .and_then(|r| r.get("value"))
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!("No visible element found containing text: \"{text}\""),
                    None,
                )
            })?;

        if coords.is_null() {
            return Err(McpError::invalid_params(
                format!("No visible element found containing text: \"{text}\""),
                None,
            ));
        }

        let x = coords.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = coords.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let matched = coords
            .get("matched")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)");

        self.execute_seq_reconnect(commands::click(x, y))
            .await
            .map_err(|e| McpError::internal_error(format!("Click failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Clicked element with text \"{matched}\" at ({x:.0}, {y:.0})"
        ))]))
    }

    #[tool(description = "Type text into an element on the page. Focuses the element first, then types character by character.")]
    async fn type_text(
        &self,
        Parameters(TypeTextParams { selector, text, clear }): Parameters<TypeTextParams>,
    ) -> Result<CallToolResult, McpError> {
        let should_clear = clear.unwrap_or(false);
        let js = js_focus_visible_element(&selector, should_clear);

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to focus element: {e}"), None))?;

        let focused = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !focused {
            return Err(McpError::invalid_params(
                format!("Element not found or not focusable: {selector}"),
                None,
            ));
        }

        // Type each character (replaces selected text if clear was used)
        self.execute_seq_reconnect(commands::type_text(&text))
            .await
            .map_err(|e| McpError::internal_error(format!("Type failed: {e}"), None))?;

        let action = if should_clear { "Cleared and typed" } else { "Typed" };
        Ok(CallToolResult::success(vec![Content::text(format!(
            "{action} {len} characters into '{selector}'",
            len = text.len()
        ))]))
    }

    #[tool(description = "Fill multiple form fields at once. Takes a JSON object mapping CSS selectors to values. Each field is focused, cleared, and typed into.")]
    async fn fill_form(
        &self,
        Parameters(FillFormParams { selector, fields }): Parameters<FillFormParams>,
    ) -> Result<CallToolResult, McpError> {
        let fields_json = serde_json::to_string(&fields).unwrap();
        let js = format!(
            r#"(async () => {{
                const container = document.querySelector({sel});
                if (!container) return {{ error: "Container not found" }};
                const fields = {fields};
                const results = [];
                for (const [fieldSel, value] of Object.entries(fields)) {{
                    const el = container.querySelector(fieldSel) || document.querySelector(fieldSel);
                    if (!el) {{
                        results.push({{ field: fieldSel, status: "not_found" }});
                        continue;
                    }}
                    el.scrollIntoView({{ block: 'center', behavior: 'instant' }});
                    await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
                    el.focus();
                    el.select();
                    results.push({{ field: fieldSel, status: "focused" }});
                }}
                return {{ ok: true, count: results.length, results: results }};
            }})()"#,
            sel = serde_json::to_string(&selector).unwrap(),
            fields = fields_json
        );

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Fill form failed: {e}"), None))?;

        let value = result
            .get("result")
            .and_then(|r| r.get("value"));

        if let Some(v) = value {
            if v.get("error").is_some() {
                return Err(McpError::invalid_params(
                    format!("Container not found: {selector}"),
                    None,
                ));
            }
        }

        // Now type into each field sequentially
        let mut filled = Vec::new();
        for (field_sel, field_value) in &fields {
            // Focus the field
            let focus_js = format!(
                r#"(() => {{
                    const container = document.querySelector({sel});
                    const el = container ? (container.querySelector({field}) || document.querySelector({field})) : document.querySelector({field});
                    if (!el) return false;
                    el.focus();
                    el.select();
                    return true;
                }})()"#,
                sel = serde_json::to_string(&selector).unwrap(),
                field = serde_json::to_string(field_sel).unwrap()
            );

            let focus_result = self.execute_reconnect(commands::evaluate(&focus_js))
                .await
                .map_err(|e| McpError::internal_error(format!("Focus failed: {e}"), None))?;

            let focused = focus_result
                .get("result")
                .and_then(|r| r.get("value"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if focused {
                self.execute_seq_reconnect(commands::type_text(field_value))
                    .await
                    .map_err(|e| McpError::internal_error(format!("Type failed: {e}"), None))?;
                filled.push(format!("{field_sel}: \"{field_value}\""));
            } else {
                filled.push(format!("{field_sel}: NOT FOUND"));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Filled {} fields:\n{}",
            filled.len(),
            filled.join("\n")
        ))]))
    }

    #[tool(description = "Wait for an element matching a CSS selector to appear in the DOM.")]
    async fn wait_for(
        &self,
        Parameters(WaitForParams {
            selector,
            timeout_ms,
        }): Parameters<WaitForParams>,
    ) -> Result<CallToolResult, McpError> {
        let timeout = timeout_ms.unwrap_or(5000);
        let interval = 200u64;
        let max_attempts = timeout / interval;

        for _ in 0..max_attempts {
            let js = format!(
                "document.querySelector({sel}) !== null",
                sel = serde_json::to_string(&selector).unwrap()
            );

            let result = self.execute_reconnect(commands::evaluate(&js))
                .await
                .map_err(|e| McpError::internal_error(format!("Wait check failed: {e}"), None))?;

            let found = result
                .get("result")
                .and_then(|r| r.get("value"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if found {
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Element '{selector}' found"
                ))]));
            }

            tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
        }

        Err(McpError::internal_error(
            format!("Timeout waiting for '{selector}' after {timeout}ms"),
            None,
        ))
    }

    #[tool(description = "Scroll the page by a given number of pixels.")]
    async fn scroll(
        &self,
        Parameters(ScrollParams { x, y }): Parameters<ScrollParams>,
    ) -> Result<CallToolResult, McpError> {
        let scroll_x = x.unwrap_or(0.0);
        let scroll_y = y.unwrap_or(0.0);

        self.execute_reconnect(commands::scroll(scroll_x, scroll_y))
            .await
            .map_err(|e| McpError::internal_error(format!("Scroll failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Scrolled by ({scroll_x:.0}, {scroll_y:.0})"
        ))]))
    }

    #[tool(description = "Hover over an element by CSS selector. Useful for revealing dropdown menus, tooltips, or hover states.")]
    async fn hover(
        &self,
        Parameters(HoverParams { selector }): Parameters<HoverParams>,
    ) -> Result<CallToolResult, McpError> {
        let js = js_find_visible_element(&selector);

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to find element: {e}"), None))?;

        let coords = result
            .get("result")
            .and_then(|r| r.get("value"));

        match coords {
            Some(v) if !v.is_null() => {
                let x = v.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y = v.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);

                self.execute_reconnect(commands::hover(x, y))
                    .await
                    .map_err(|e| McpError::internal_error(format!("Hover failed: {e}"), None))?;

                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Hovered over '{selector}' at ({x:.0}, {y:.0})"
                ))]))
            }
            _ => Err(McpError::invalid_params(
                format!("No visible, in-viewport element found for: {selector}"),
                None,
            )),
        }
    }

    #[tool(description = "Press a keyboard key (Enter, Tab, Escape, ArrowDown, Backspace, Space, etc.). Useful for form submission, navigation, and closing dialogs.")]
    async fn press_key(
        &self,
        Parameters(PressKeyParams { key }): Parameters<PressKeyParams>,
    ) -> Result<CallToolResult, McpError> {
        self.execute_seq_reconnect(commands::press_key(&key))
            .await
            .map_err(|e| McpError::internal_error(format!("Key press failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Pressed key: {key}"
        ))]))
    }

    #[tool(description = "Read an attribute value from the first matching element. Useful for getting href, src, data-* attributes, or form values.")]
    async fn get_attribute(
        &self,
        Parameters(GetAttributeParams { selector, attribute }): Parameters<GetAttributeParams>,
    ) -> Result<CallToolResult, McpError> {
        let js = format!(
            r#"(() => {{
                const el = document.querySelector({sel});
                if (!el) return null;
                return el.getAttribute({attr});
            }})()"#,
            sel = serde_json::to_string(&selector).unwrap(),
            attr = serde_json::to_string(&attribute).unwrap()
        );

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to get attribute: {e}"), None))?;

        let value = result
            .get("result")
            .and_then(|r| r.get("value"));

        match value {
            Some(v) if !v.is_null() => {
                let attr_value = v.as_str().unwrap_or("(non-string value)");
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "{attribute}=\"{attr_value}\""
                ))]))
            }
            _ => Err(McpError::invalid_params(
                format!("Element '{selector}' not found or attribute '{attribute}' not present"),
                None,
            )),
        }
    }

    #[tool(description = "Get the current page URL and title without navigating.")]
    async fn get_url(&self) -> Result<CallToolResult, McpError> {
        let result = self.execute_reconnect(
            commands::evaluate("JSON.stringify({ url: window.location.href, title: document.title })"),
        )
        .await
        .map_err(|e| McpError::internal_error(format!("Get URL failed: {e}"), None))?;

        let value = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("{}");

        let parsed: serde_json::Value = serde_json::from_str(value).unwrap_or_default();
        let url = parsed.get("url").and_then(|v| v.as_str()).unwrap_or("(unknown)");
        let title = parsed.get("title").and_then(|v| v.as_str()).unwrap_or("(unknown)");

        Ok(CallToolResult::success(vec![Content::text(format!(
            "URL: {url}\nTitle: {title}"
        ))]))
    }

    #[tool(description = "Wait for a page navigation to complete (e.g. after clicking a link). Polls document.readyState until 'complete'. Useful for SPAs and full page loads.")]
    async fn wait_for_navigation(
        &self,
        Parameters(WaitForNavigationParams { timeout_ms }): Parameters<WaitForNavigationParams>,
    ) -> Result<CallToolResult, McpError> {
        let timeout = timeout_ms.unwrap_or(10000);
        let interval = 200u64;
        let max_attempts = timeout / interval;

        // Snapshot the starting URL so we can detect SPA navigations (where readyState stays 'complete').
        let start_url = self.execute_reconnect(commands::evaluate("window.location.href"))
            .await
            .ok()
            .and_then(|r| r.get("result")?.get("value")?.as_str().map(|s| s.to_owned()))
            .unwrap_or_default();

        let mut nav_started = false;

        for _ in 0..max_attempts {
            let result = self.execute_reconnect(commands::evaluate(
                "({ state: document.readyState, url: window.location.href })"
            ))
            .await
            .map_err(|e| McpError::internal_error(format!("Navigation check failed: {e}"), None))?;

            let val = result.get("result").and_then(|r| r.get("value"));
            let state = val.and_then(|v| v.get("state")).and_then(|v| v.as_str()).unwrap_or("loading");
            let url = val.and_then(|v| v.get("url")).and_then(|v| v.as_str()).unwrap_or("");

            // Navigation has started if: URL changed (SPA) or readyState is no longer 'complete' (full nav).
            if url != start_url || state != "complete" {
                nav_started = true;
            }

            if nav_started && state == "complete" {
                let title_result = self.execute_reconnect(commands::evaluate("document.title"))
                    .await
                    .ok();

                let title = title_result
                    .as_ref()
                    .and_then(|r| r.get("result"))
                    .and_then(|r| r.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("(unknown)");

                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Page loaded: {title}"
                ))]));
            }

            tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
        }

        Err(McpError::internal_error(
            format!("Navigation did not complete within {timeout}ms"),
            None,
        ))
    }

    #[tool(description = "Navigate back in browser history.")]
    async fn back(&self) -> Result<CallToolResult, McpError> {
        self.nav_history_step(-1).await
    }

    #[tool(description = "Navigate forward in browser history.")]
    async fn forward(&self) -> Result<CallToolResult, McpError> {
        self.nav_history_step(1).await
    }

    /// Navigate back (delta = -1) or forward (delta = +1) using CDP history API.
    /// Page.navigateToHistoryEntry returns after navigation commits — no sleep needed.
    async fn nav_history_step(&self, delta: i64) -> Result<CallToolResult, McpError> {
        let history = self.execute_reconnect(commands::get_navigation_history())
            .await
            .map_err(|e| McpError::internal_error(format!("History fetch failed: {e}"), None))?;

        let current_index = history.get("currentIndex").and_then(|v| v.as_i64()).unwrap_or(0);
        let entries = history
            .get("entries")
            .and_then(|v| v.as_array())
            .ok_or_else(|| McpError::internal_error("No navigation history".to_owned(), None))?;

        let target_index = current_index + delta;
        if target_index < 0 || target_index >= entries.len() as i64 {
            let dir = if delta < 0 { "beginning" } else { "end" };
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Already at the {dir} of history"
            ))]));
        }

        let entry_id = entries[target_index as usize]
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| McpError::internal_error("Invalid history entry".to_owned(), None))?;

        // navigateToHistoryEntry returns after navigation commits — then wait for load.
        self.execute_reconnect(commands::navigate_to_history_entry(entry_id))
            .await
            .map_err(|e| McpError::internal_error(format!("Navigation failed: {e}"), None))?;

        let _ = self.execute_reconnect(commands::evaluate(
            "new Promise(resolve => {
                if (document.readyState === 'complete') { resolve(); return; }
                window.addEventListener('load', () => resolve(), { once: true });
                setTimeout(resolve, 8000);
            })",
        ))
        .await;

        let dir = if delta < 0 { "back" } else { "forward" };
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Navigated {dir}"
        ))]))
    }

    #[tool(description = "Select an option in a <select> dropdown by its value attribute.")]
    async fn select_option(
        &self,
        Parameters(SelectOptionParams { selector, value }): Parameters<SelectOptionParams>,
    ) -> Result<CallToolResult, McpError> {
        let js = format!(
            r#"(() => {{
                const el = document.querySelector({sel});
                if (!el) return "not_found";
                el.value = {val};
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return "ok";
            }})()"#,
            sel = serde_json::to_string(&selector).unwrap(),
            val = serde_json::to_string(&value).unwrap()
        );

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Select failed: {e}"), None))?;

        let status = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        if status == "not_found" {
            return Err(McpError::invalid_params(
                format!("Select element not found: {selector}"),
                None,
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Selected value '{value}' in '{selector}'"
        ))]))
    }

    #[tool(description = "Submit a form element by CSS selector.")]
    async fn submit_form(
        &self,
        Parameters(ClickParams { selector }): Parameters<ClickParams>,
    ) -> Result<CallToolResult, McpError> {
        let js = format!(
            r#"(() => {{
                const el = document.querySelector({sel});
                if (!el) return "not_found";
                if (el.tagName === 'FORM') {{
                    el.submit();
                }} else {{
                    const form = el.closest('form');
                    if (form) form.submit();
                    else return "no_form";
                }}
                return "ok";
            }})()"#,
            sel = serde_json::to_string(&selector).unwrap()
        );

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Submit failed: {e}"), None))?;

        let status = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        match status {
            "not_found" => Err(McpError::invalid_params(
                format!("Element not found: {selector}"),
                None,
            )),
            "no_form" => Err(McpError::invalid_params(
                format!("No parent form found for: {selector}"),
                None,
            )),
            _ => Ok(CallToolResult::success(vec![Content::text(
                "Form submitted".to_owned(),
            )])),
        }
    }

    #[tool(description = "Discover WebMCP tools declared by the current page via navigator.modelContext. Returns structured tool definitions if the page exposes any.")]
    async fn discover_webmcp_tools(&self) -> Result<CallToolResult, McpError> {
        let js = r#"(() => {
            if (!navigator.modelContext) return { supported: false };
            const tools = navigator.modelContext.tools || [];
            return {
                supported: true,
                tools: tools.map(t => ({
                    name: t.name,
                    description: t.description,
                    inputSchema: t.inputSchema,
                }))
            };
        })()"#;

        let result = self.execute_reconnect(commands::evaluate(js))
            .await
            .map_err(|e| McpError::internal_error(format!("WebMCP check failed: {e}"), None))?;

        let value = result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let supported = value
            .get("supported")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !supported {
            return Ok(CallToolResult::success(vec![Content::text(
                "This page does not support WebMCP (navigator.modelContext not available)".to_owned(),
            )]));
        }

        let tools = value.get("tools").cloned().unwrap_or(serde_json::json!([]));
        let tool_count = tools.as_array().map(|a| a.len()).unwrap_or(0);

        if tool_count == 0 {
            return Ok(CallToolResult::success(vec![Content::text(
                "WebMCP is available but no tools are registered on this page".to_owned(),
            )]));
        }

        let formatted = serde_json::to_string_pretty(&tools)
            .unwrap_or_else(|_| format!("{tools:?}"));

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Found {tool_count} WebMCP tool(s):\n\n{formatted}"
        ))]))
    }

    #[tool(description = "Inspect the DOM tree starting from a CSS selector. Returns a compact structural view with tag names, key attributes (id, class, href, type, name, value, role, aria-label), and truncated text content. Essential for understanding page structure without screenshots.")]
    async fn inspect(
        &self,
        Parameters(InspectParams { selector, max_depth }): Parameters<InspectParams>,
    ) -> Result<CallToolResult, McpError> {
        let sel = selector.as_deref().unwrap_or("body");
        let depth = max_depth.unwrap_or(4);

        let js = format!(
            r#"(() => {{
                const ATTRS = ['id','class','href','src','type','name','value','role','aria-label','placeholder','action','method','for','alt','title'];
                const MAX_TEXT = 80;
                const MAX_CHILDREN = 50;

                function walk(el, depth, maxDepth) {{
                    if (depth > maxDepth) return '  '.repeat(depth) + '...';
                    const tag = el.tagName.toLowerCase();
                    let attrs = '';
                    for (const a of ATTRS) {{
                        const v = el.getAttribute(a);
                        if (v) attrs += ' ' + a + '="' + v.substring(0, 60) + '"';
                    }}
                    const indent = '  '.repeat(depth);
                    let result = indent + '<' + tag + attrs + '>';

                    const children = el.children;
                    const textContent = el.childNodes.length === 1 && el.childNodes[0].nodeType === 3
                        ? el.childNodes[0].textContent.trim() : null;

                    if (textContent && textContent.length > 0) {{
                        const t = textContent.length > MAX_TEXT
                            ? textContent.substring(0, MAX_TEXT) + '...' : textContent;
                        result += t + '</' + tag + '>';
                        return result;
                    }}

                    if (children.length === 0) {{
                        const t = el.textContent.trim();
                        if (t.length > 0) {{
                            const truncated = t.length > MAX_TEXT ? t.substring(0, MAX_TEXT) + '...' : t;
                            result += truncated + '</' + tag + '>';
                        }} else {{
                            result = indent + '<' + tag + attrs + ' />';
                        }}
                        return result;
                    }}

                    result += '\n';
                    const len = Math.min(children.length, MAX_CHILDREN);
                    for (let i = 0; i < len; i++) {{
                        result += walk(children[i], depth + 1, maxDepth) + '\n';
                    }}
                    if (children.length > MAX_CHILDREN) {{
                        result += indent + '  ... +' + (children.length - MAX_CHILDREN) + ' more\n';
                    }}
                    result += indent + '</' + tag + '>';
                    return result;
                }}

                const root = document.querySelector({sel});
                if (!root) return null;
                return walk(root, 0, {depth});
            }})()"#,
            sel = serde_json::to_string(sel).unwrap(),
            depth = depth
        );

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Inspect failed: {e}"), None))?;

        let value = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str());

        match value {
            Some(tree) => {
                let truncated = if tree.len() > 15000 {
                    format!("{}...\n\n[Truncated — {} total chars]", &tree[..15000], tree.len())
                } else {
                    tree.to_owned()
                };
                Ok(CallToolResult::success(vec![Content::text(truncated)]))
            }
            None => Err(McpError::invalid_params(
                format!("Element not found: {sel}"),
                None,
            )),
        }
    }

    #[tool(description = "Find all elements matching a CSS selector and return their tag, text content, key attributes, and count. Useful for finding interactive elements, links, buttons, form fields, etc.")]
    async fn query_elements(
        &self,
        Parameters(QueryElementsParams { selector }): Parameters<QueryElementsParams>,
    ) -> Result<CallToolResult, McpError> {
        let js = format!(
            r#"(() => {{
                const els = document.querySelectorAll({sel});
                const results = [];
                const MAX = 50;
                for (let i = 0; i < Math.min(els.length, MAX); i++) {{
                    const el = els[i];
                    const rect = el.getBoundingClientRect();
                    const entry = {{
                        index: i,
                        tag: el.tagName.toLowerCase(),
                        text: el.textContent.trim().substring(0, 100),
                        id: el.id || undefined,
                        class: el.className || undefined,
                        href: el.getAttribute('href') || undefined,
                        type: el.getAttribute('type') || undefined,
                        name: el.getAttribute('name') || undefined,
                        value: el.value || undefined,
                        visible: rect.width > 0 && rect.height > 0,
                    }};
                    // Remove undefined keys
                    Object.keys(entry).forEach(k => entry[k] === undefined && delete entry[k]);
                    results.push(entry);
                }}
                return {{ total: els.length, shown: Math.min(els.length, MAX), elements: results }};
            }})()"#,
            sel = serde_json::to_string(&selector).unwrap()
        );

        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Query failed: {e}"), None))?;

        let value = result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let output = serde_json::to_string_pretty(&value)
            .unwrap_or_else(|_| format!("{value:?}"));

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "List all open browser tabs with their titles, URLs, and target IDs.")]
    async fn list_tabs(&self) -> Result<CallToolResult, McpError> {
        let url = format!("http://localhost:{}/json", self.port);
        let client = reqwest::Client::new();

        let targets: Vec<serde_json::Value> = client
            .get(&url)
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to list tabs: {e}"), None))?
            .json()
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to parse tabs: {e}"), None))?;

        // Get current page URL to mark the active CDP tab
        let current_url = self.execute_reconnect(commands::evaluate("window.location.href"))
            .await
            .ok()
            .and_then(|r| r.get("result")?.get("value")?.as_str().map(|s| s.to_owned()));

        let mut output = String::new();
        for target in &targets {
            if target.get("type").and_then(|t| t.as_str()) == Some("page") {
                let id = target.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let title = target.get("title").and_then(|v| v.as_str()).unwrap_or("(untitled)");
                let tab_url = target.get("url").and_then(|v| v.as_str()).unwrap_or("?");
                let active = current_url.as_deref() == Some(tab_url);
                let marker = if active { " *" } else { "" };
                output.push_str(&format!("[{id}]{marker} {title}\n  {tab_url}\n\n"));
            }
        }

        if output.is_empty() {
            output = "No open tabs found".to_owned();
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Execute a CDP command, retrying once with reconnect on connection failure.
    async fn exec_with_reconnect(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, cdp::CdpError> {
        match cdp::send(&*self.live.get().await, method, params.clone()).await {
            Ok(val) => Ok(val),
            Err(cdp::CdpError::SendFailed) | Err(cdp::CdpError::ResponseDropped) | Err(cdp::CdpError::Timeout) => {
                self.try_reconnect().await.map_err(|msg| cdp::CdpError::ConnectionFailed(msg))?;
                cdp::send(&*self.live.get().await, method, params).await
            }
            Err(e) => Err(e),
        }
    }

    /// Execute a CDP command (built by commands.rs) with reconnect on failure.
    async fn execute_reconnect(&self, command: (&str, serde_json::Value)) -> Result<serde_json::Value, cdp::CdpError> {
        let (method, params) = command;
        self.exec_with_reconnect(method, params).await
    }

    /// Execute a CDP command sequence with reconnect on failure.
    async fn execute_seq_reconnect(&self, commands: Vec<(&'static str, serde_json::Value)>) -> Result<serde_json::Value, cdp::CdpError> {
        match cdp::execute_sequence(&*self.live.get().await, commands.clone()).await {
            Ok(val) => Ok(val),
            Err(cdp::CdpError::SendFailed) | Err(cdp::CdpError::ResponseDropped) | Err(cdp::CdpError::Timeout) => {
                self.try_reconnect().await.map_err(|msg| cdp::CdpError::ConnectionFailed(msg))?;
                cdp::execute_sequence(&*self.live.get().await, commands).await
            }
            Err(e) => Err(e),
        }
    }

    /// Reconnect CDP — returns to the sticky target if one is set, otherwise any page.
    async fn try_reconnect(&self) -> Result<(), String> {
        tracing::info!("Attempting CDP reconnect...");
        let sticky = self.sticky_target.lock().await.clone();
        let ws_url = crate::browser::find_target_ws_url(self.port, sticky.as_deref())
            .await
            .map_err(|e| format!("No browser target available: {e}"))?;
        let new_conn = cdp::connect_to_target(&ws_url)
            .await
            .map_err(|e| format!("Reconnect failed: {e}"))?;
        self.live.swap(new_conn).await;
        self.resubscribe_events().await;
        tracing::info!("CDP reconnected to {ws_url}");
        Ok(())
    }

    /// Reconnect CDP to a specific target by ID.
    async fn reconnect_to_target(&self, target_id: &str) -> Result<(), McpError> {
        let ws_url = crate::browser::find_target_ws_url(self.port, Some(target_id))
            .await
            .map_err(|e| McpError::internal_error(
                format!("Could not find WebSocket URL for target {target_id}: {e}"),
                None,
            ))?;
        let new_conn = cdp::connect_to_target(&ws_url)
            .await
            .map_err(|e| McpError::internal_error(
                format!("Failed to connect to target {target_id}: {e}"),
                None,
            ))?;
        self.live.swap(new_conn).await;
        self.resubscribe_events().await;
        Ok(())
    }

    #[tool(description = "Switch to a browser tab by its target ID (from list_tabs).")]
    async fn switch_tab(
        &self,
        Parameters(SwitchTabParams { target_id }): Parameters<SwitchTabParams>,
    ) -> Result<CallToolResult, McpError> {
        // Visually activate the tab
        cdp::send(
            &*self.live.get().await,
            "Target.activateTarget",
            serde_json::json!({ "targetId": target_id }),
        )
        .await
        .map_err(|e| McpError::internal_error(format!("Activate tab failed: {e}"), None))?;

        // Reconnect CDP to the new tab's target and pin it as sticky.
        self.reconnect_to_target(&target_id).await?;
        *self.sticky_target.lock().await = Some(target_id.clone());

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Switched to tab {target_id} (CDP reconnected)"
        ))]))
    }

    #[tool(description = "Open a new browser tab, optionally with a URL.")]
    async fn new_tab(
        &self,
        Parameters(NewTabParams { url }): Parameters<NewTabParams>,
    ) -> Result<CallToolResult, McpError> {
        let target_url = url.as_deref().unwrap_or("about:blank");

        let result = cdp::send(
            &*self.live.get().await,
            "Target.createTarget",
            serde_json::json!({ "url": target_url }),
        )
        .await
        .map_err(|e| McpError::internal_error(format!("New tab failed: {e}"), None))?;

        let target_id = result
            .get("targetId")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)")
            .to_owned();

        // Give the new tab a moment to register its debug endpoint
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Reconnect CDP to the new tab and pin it as sticky.
        self.reconnect_to_target(&target_id).await?;
        *self.sticky_target.lock().await = Some(target_id.clone());

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Opened and switched to new tab [{target_id}]: {target_url}"
        ))]))
    }

    #[tool(description = "Close a browser tab by its target ID (from list_tabs).")]
    async fn close_tab(
        &self,
        Parameters(CloseTabParams { target_id }): Parameters<CloseTabParams>,
    ) -> Result<CallToolResult, McpError> {
        cdp::send(
            &*self.live.get().await,
            "Target.closeTarget",
            serde_json::json!({ "targetId": target_id }),
        )
        .await
        .map_err(|e| McpError::internal_error(format!("Close tab failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Closed tab {target_id}"
        ))]))
    }

    // ---- Batch 1: New interaction tools ----

    #[tool(description = "Handle a browser dialog (alert, confirm, prompt, or beforeunload). Use this when a dialog is blocking the page.")]
    async fn handle_dialog(
        &self,
        Parameters(HandleDialogParams { accept, prompt_text }): Parameters<HandleDialogParams>,
    ) -> Result<CallToolResult, McpError> {
        self.execute_reconnect(commands::handle_dialog(accept, prompt_text.as_deref()))
            .await
            .map_err(|e| McpError::internal_error(format!("Handle dialog failed: {e}"), None))?;

        let action = if accept { "accepted" } else { "dismissed" };
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Dialog {action}"
        ))]))
    }

    #[tool(description = "Press a keyboard shortcut with modifier keys (e.g. \"Ctrl+A\", \"Ctrl+Shift+T\", \"Alt+F4\"). Use modifier names: Ctrl, Alt, Shift, Meta.")]
    async fn keyboard_chord(
        &self,
        Parameters(KeyboardChordParams { chord }): Parameters<KeyboardChordParams>,
    ) -> Result<CallToolResult, McpError> {
        let (modifiers, key) = parse_chord(&chord);
        self.execute_seq_reconnect(commands::key_chord(&key, modifiers))
            .await
            .map_err(|e| McpError::internal_error(format!("Keyboard chord failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Pressed chord: {chord}"
        ))]))
    }

    #[tool(description = "Double-click an element by CSS selector. Useful for selecting text or triggering double-click handlers.")]
    async fn double_click(
        &self,
        Parameters(DoubleClickParams { selector }): Parameters<DoubleClickParams>,
    ) -> Result<CallToolResult, McpError> {
        let js = js_find_visible_element(&selector);
        let result = self.execute_reconnect(commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to find element: {e}"), None))?;

        let coords = result.get("result").and_then(|r| r.get("value"));
        match coords {
            Some(v) if !v.is_null() => {
                let x = v.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y = v.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                self.execute_seq_reconnect(commands::double_click(x, y))
                    .await
                    .map_err(|e| McpError::internal_error(format!("Double-click failed: {e}"), None))?;
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Double-clicked '{selector}' at ({x:.0}, {y:.0})"
                ))]))
            }
            _ => Err(McpError::invalid_params(
                format!("No visible, in-viewport element found for: {selector}"),
                None,
            )),
        }
    }

    #[tool(description = "Drag from one location to another. Provide CSS selectors (preferred) or explicit x/y coordinates.")]
    async fn drag(
        &self,
        #[allow(clippy::too_many_arguments)]
        Parameters(DragParams { from_selector, to_selector, from_x, from_y, to_x, to_y, steps }): Parameters<DragParams>,
    ) -> Result<CallToolResult, McpError> {
        // Resolve "from" coordinates
        let (fx, fy) = if let Some(sel) = &from_selector {
            let js = js_find_visible_element(sel);
            let result = self.execute_reconnect(commands::evaluate(&js))
                .await
                .map_err(|e| McpError::internal_error(format!("Failed to find from element: {e}"), None))?;
            let v = result.get("result").and_then(|r| r.get("value"))
                .filter(|v| !v.is_null())
                .ok_or_else(|| McpError::invalid_params(format!("No visible element for from_selector: {sel}"), None))?;
            (
                v.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0),
                v.get("y").and_then(|y| y.as_f64()).unwrap_or(0.0),
            )
        } else {
            (
                from_x.ok_or_else(|| McpError::invalid_params("Provide from_selector or from_x+from_y".to_owned(), None))?,
                from_y.ok_or_else(|| McpError::invalid_params("Provide from_selector or from_x+from_y".to_owned(), None))?,
            )
        };

        // Resolve "to" coordinates
        let (tx, ty) = if let Some(sel) = &to_selector {
            let js = js_find_visible_element(sel);
            let result = self.execute_reconnect(commands::evaluate(&js))
                .await
                .map_err(|e| McpError::internal_error(format!("Failed to find to element: {e}"), None))?;
            let v = result.get("result").and_then(|r| r.get("value"))
                .filter(|v| !v.is_null())
                .ok_or_else(|| McpError::invalid_params(format!("No visible element for to_selector: {sel}"), None))?;
            (
                v.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0),
                v.get("y").and_then(|y| y.as_f64()).unwrap_or(0.0),
            )
        } else {
            (
                to_x.ok_or_else(|| McpError::invalid_params("Provide to_selector or to_x+to_y".to_owned(), None))?,
                to_y.ok_or_else(|| McpError::invalid_params("Provide to_selector or to_x+to_y".to_owned(), None))?,
            )
        };

        let drag_steps = steps.unwrap_or(10);
        self.execute_seq_reconnect(commands::drag(fx, fy, tx, ty, drag_steps))
            .await
            .map_err(|e| McpError::internal_error(format!("Drag failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Dragged from ({fx:.0}, {fy:.0}) to ({tx:.0}, {ty:.0})"
        ))]))
    }

    #[tool(description = "Set the browser viewport size. Useful for testing responsive layouts or ensuring consistent screenshots.")]
    async fn set_viewport(
        &self,
        Parameters(SetViewportParams { width, height }): Parameters<SetViewportParams>,
    ) -> Result<CallToolResult, McpError> {
        self.execute_reconnect(commands::set_viewport(width, height))
            .await
            .map_err(|e| McpError::internal_error(format!("Set viewport failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Viewport set to {width}x{height}"
        ))]))
    }

    #[tool(description = "Get a snapshot of the page's accessibility tree. Works on all pages — browsers compute the AX tree from semantic HTML even without explicit ARIA. Returns a compact indented role/name tree, much more token-efficient than screenshots for navigation.")]
    async fn accessibility_snapshot(&self) -> Result<CallToolResult, McpError> {
        // Enable Accessibility domain (idempotent)
        let _ = self.execute_reconnect(commands::enable_accessibility()).await;

        let result = self.execute_reconnect(commands::get_full_ax_tree())
            .await
            .map_err(|e| McpError::internal_error(format!("Accessibility snapshot failed: {e}"), None))?;

        let nodes = result
            .get("nodes")
            .and_then(|n| n.as_array())
            .ok_or_else(|| McpError::internal_error("No accessibility nodes returned".to_owned(), None))?;

        let tree = render_ax_tree(nodes);
        let truncated = if tree.len() > 15000 {
            format!("{}...\n\n[Truncated — {} total chars]", &tree[..15000], tree.len())
        } else {
            tree
        };

        Ok(CallToolResult::success(vec![Content::text(truncated)]))
    }

    // ---- Batch 2: Event buffering tools ----

    #[tool(description = "Read buffered console messages from the page. Includes console.log, warn, error, etc. Messages accumulate since last navigation or clear.")]
    async fn get_console_messages(
        &self,
        Parameters(GetConsoleMessagesParams { level, clear }): Parameters<GetConsoleMessagesParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut log = self.console_log.lock().await;
        let filtered: Vec<&ConsoleEntry> = log
            .iter()
            .filter(|e| level.as_deref().map(|l| e.level == l).unwrap_or(true))
            .collect();

        if filtered.is_empty() {
            if clear.unwrap_or(false) { log.clear(); }
            return Ok(CallToolResult::success(vec![Content::text(
                "No console messages".to_owned(),
            )]));
        }

        let count = filtered.len();
        let output = filtered
            .iter()
            .map(|e| format!("[{}] {}", e.level.to_uppercase(), e.text))
            .collect::<Vec<_>>()
            .join("\n");

        if clear.unwrap_or(false) { log.clear(); }

        Ok(CallToolResult::success(vec![Content::text(format!(
            "{count} message(s):\n{output}"
        ))]))
    }

    #[tool(description = "List buffered network requests captured since last navigation. Shows method, URL, and HTTP status. Optionally filter by URL substring.")]
    async fn list_network_requests(
        &self,
        Parameters(ListNetworkRequestsParams { url_filter, clear }): Parameters<ListNetworkRequestsParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut log = self.network_log.lock().await;
        let filtered: Vec<&NetworkEntry> = log
            .iter()
            .filter(|e| {
                url_filter
                    .as_deref()
                    .map(|f| e.url.to_lowercase().contains(&f.to_lowercase()))
                    .unwrap_or(true)
            })
            .collect();

        if filtered.is_empty() {
            if clear.unwrap_or(false) { log.clear(); }
            return Ok(CallToolResult::success(vec![Content::text(
                "No network requests captured".to_owned(),
            )]));
        }

        let count = filtered.len();
        let output = filtered
            .iter()
            .map(|e| {
                let status = e.status.map(|s| format!("{s}")).unwrap_or_else(|| "pending".to_owned());
                format!("{} {} [{status}]", e.method, e.url)
            })
            .collect::<Vec<_>>()
            .join("\n");

        if clear.unwrap_or(false) { log.clear(); }

        Ok(CallToolResult::success(vec![Content::text(format!(
            "{count} request(s):\n{output}"
        ))]))
    }

    // ---- Event subscription (non-tool methods) ----

    /// Subscribe to CDP events from the current connection and spawn a collector task.
    /// Old collector tasks die naturally when their connection's broadcast sender drops.
    pub async fn resubscribe_events(&self) {
        let receiver = {
            let conn = self.live.get().await;
            cdp::subscribe_events(&*conn)
        };
        let console_log = self.console_log.clone();
        let network_log = self.network_log.clone();
        tokio::spawn(Self::run_event_collector(receiver, console_log, network_log));
    }

    async fn run_event_collector(
        mut receiver: tokio::sync::broadcast::Receiver<cdp::CdpEvent>,
        console_log: Arc<tokio::sync::Mutex<Vec<ConsoleEntry>>>,
        network_log: Arc<tokio::sync::Mutex<Vec<NetworkEntry>>>,
    ) {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    match event.method.as_str() {
                        "Runtime.consoleAPICalled" => {
                            let level = event.params
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("log")
                                .to_owned();
                            let text = event.params
                                .get("args")
                                .and_then(|a| a.as_array())
                                .map(|args| {
                                    args.iter()
                                        .filter_map(|a| {
                                            a.get("value")
                                                .and_then(|v| v.as_str())
                                                .map(|s| s.to_owned())
                                                .or_else(|| {
                                                    a.get("description")
                                                        .and_then(|v| v.as_str())
                                                        .map(|s| s.to_owned())
                                                })
                                                .or_else(|| Some(serde_json::to_string(a).unwrap_or_default()))
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                })
                                .unwrap_or_default();
                            let timestamp = event.params
                                .get("timestamp")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            console_log.lock().await.push(ConsoleEntry { level, text, timestamp });
                        }
                        "Network.requestWillBeSent" => {
                            let request_id = event.params
                                .get("requestId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_owned();
                            let url = event.params
                                .get("request")
                                .and_then(|r| r.get("url"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_owned();
                            let method = event.params
                                .get("request")
                                .and_then(|r| r.get("method"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("GET")
                                .to_owned();
                            let timestamp = event.params
                                .get("timestamp")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            network_log.lock().await.push(NetworkEntry {
                                request_id, url, method, status: None, timestamp,
                            });
                        }
                        "Network.responseReceived" => {
                            let request_id = event.params
                                .get("requestId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let status = event.params
                                .get("response")
                                .and_then(|r| r.get("status"))
                                .and_then(|v| v.as_u64())
                                .map(|s| s as u16);
                            let mut log = network_log.lock().await;
                            for entry in log.iter_mut().rev() {
                                if entry.request_id == request_id {
                                    entry.status = status;
                                    break;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }
}

#[tool_handler]
impl ServerHandler for CausewayServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Causeway — sovereign browser bridge. \
                 Drives a real Chromium browser via Chrome DevTools Protocol. \
                 Navigate, screenshot, click, type, read pages, execute JS."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
