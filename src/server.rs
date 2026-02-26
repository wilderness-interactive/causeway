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
pub struct GetCookiesParams {
    #[schemars(description = "Optional URL filter — only return cookies for this domain. If omitted, returns cookies for the current page.")]
    pub url: Option<String>,
}

// -- MCP Server --

#[derive(Debug, Clone)]
pub struct CausewayServer {
    live: Arc<LiveConnection>,
    port: u16,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CausewayServer {
    pub fn new(live: Arc<LiveConnection>, port: u16) -> Self {
        Self {
            live,
            port,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Navigate the browser to a URL. Returns the page title after loading.")]
    async fn navigate(
        &self,
        Parameters(NavigateParams { url }): Parameters<NavigateParams>,
    ) -> Result<CallToolResult, McpError> {
        self.execute_reconnect(commands::navigate(&url))
            .await
            .map_err(|e| McpError::internal_error(format!("Navigate failed: {e}"), None))?;

        // Wait a moment for the page to settle, then get the title
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

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

        let result = cdp::execute(&*self.live.get().await, commands::evaluate(&js))
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

        let result = cdp::send(&*self.live.get().await, "Network.getCookies", params)
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

    #[tool(description = "Execute JavaScript in the page context and return the result.")]
    async fn evaluate_js(
        &self,
        Parameters(EvaluateJsParams { expression }): Parameters<EvaluateJsParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = cdp::execute(&*self.live.get().await, commands::evaluate(&expression))
            .await
            .map_err(|e| McpError::internal_error(format!("JS evaluation failed: {e}"), None))?;

        // Check for exceptions
        if let Some(exception) = result.get("exceptionDetails") {
            let msg = exception
                .get("exception")
                .and_then(|e| e.get("description"))
                .and_then(|d| d.as_str())
                .unwrap_or("Unknown JS error");
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
        // Find first visible, in-viewport element matching the selector
        let js = format!(
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
            sel = serde_json::to_string(&selector).unwrap()
        );

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

        cdp::execute_sequence(&*self.live.get().await, commands::click(x, y))
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

        cdp::execute_sequence(&*self.live.get().await, commands::click(x, y))
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
        // Find the first visible matching element, focus it
        let should_clear = clear.unwrap_or(false);
        let js = format!(
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
            sel = serde_json::to_string(&selector).unwrap(),
            clear = should_clear
        );

        let result = cdp::execute(&*self.live.get().await, commands::evaluate(&js))
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
        cdp::execute_sequence(&*self.live.get().await, commands::type_text(&text))
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

        let result = cdp::execute(&*self.live.get().await, commands::evaluate(&js))
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

            let focus_result = cdp::execute(&*self.live.get().await, commands::evaluate(&focus_js))
                .await
                .map_err(|e| McpError::internal_error(format!("Focus failed: {e}"), None))?;

            let focused = focus_result
                .get("result")
                .and_then(|r| r.get("value"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if focused {
                cdp::execute_sequence(&*self.live.get().await, commands::type_text(field_value))
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

            let result = cdp::execute(&*self.live.get().await, commands::evaluate(&js))
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

        cdp::execute(&*self.live.get().await, commands::scroll(scroll_x, scroll_y))
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
        // Use same smart selection as click — find first visible, in-viewport element
        let js = format!(
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
            sel = serde_json::to_string(&selector).unwrap()
        );

        let result = cdp::execute(&*self.live.get().await, commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to find element: {e}"), None))?;

        let coords = result
            .get("result")
            .and_then(|r| r.get("value"));

        match coords {
            Some(v) if !v.is_null() => {
                let x = v.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y = v.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);

                cdp::execute(&*self.live.get().await, commands::hover(x, y))
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
        cdp::execute_sequence(&*self.live.get().await, commands::press_key(&key))
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

        let result = cdp::execute(&*self.live.get().await, commands::evaluate(&js))
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

    #[tool(description = "Navigate back in browser history.")]
    async fn back(&self) -> Result<CallToolResult, McpError> {
        cdp::execute(&*self.live.get().await, commands::go_back())
            .await
            .map_err(|e| McpError::internal_error(format!("Back failed: {e}"), None))?;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        Ok(CallToolResult::success(vec![Content::text(
            "Navigated back".to_owned(),
        )]))
    }

    #[tool(description = "Navigate forward in browser history.")]
    async fn forward(&self) -> Result<CallToolResult, McpError> {
        cdp::execute(&*self.live.get().await, commands::go_forward())
            .await
            .map_err(|e| McpError::internal_error(format!("Forward failed: {e}"), None))?;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        Ok(CallToolResult::success(vec![Content::text(
            "Navigated forward".to_owned(),
        )]))
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

        let result = cdp::execute(&*self.live.get().await, commands::evaluate(&js))
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

        let result = cdp::execute(&*self.live.get().await, commands::evaluate(&js))
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

        let result = cdp::execute(&*self.live.get().await, commands::evaluate(js))
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

        let result = cdp::execute(&*self.live.get().await, commands::evaluate(&js))
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

        let result = cdp::execute(&*self.live.get().await, commands::evaluate(&js))
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

        let mut output = String::new();
        for target in &targets {
            if target.get("type").and_then(|t| t.as_str()) == Some("page") {
                let id = target.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let title = target.get("title").and_then(|v| v.as_str()).unwrap_or("(untitled)");
                let url = target.get("url").and_then(|v| v.as_str()).unwrap_or("?");
                output.push_str(&format!("[{id}] {title}\n  {url}\n\n"));
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
            Err(cdp::CdpError::SendFailed) | Err(cdp::CdpError::ResponseDropped) => {
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

    /// Reconnect CDP to any available page target.
    async fn try_reconnect(&self) -> Result<(), String> {
        tracing::info!("Attempting CDP reconnect...");
        let ws_url = crate::browser::find_target_ws_url(self.port, None)
            .await
            .map_err(|e| format!("No browser target available: {e}"))?;
        let new_conn = cdp::connect_to_target(&ws_url)
            .await
            .map_err(|e| format!("Reconnect failed: {e}"))?;
        self.live.swap(new_conn).await;
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

        // Reconnect CDP to the new tab's target
        self.reconnect_to_target(&target_id).await?;

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

        // Reconnect CDP to the new tab
        self.reconnect_to_target(&target_id).await?;

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
