use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};

use crate::cdp::{self, CdpConnection};
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

// -- MCP Server --

#[derive(Debug, Clone)]
pub struct CausewayServer {
    conn: Arc<CdpConnection>,
    port: u16,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CausewayServer {
    pub fn new(conn: Arc<CdpConnection>, port: u16) -> Self {
        Self {
            conn,
            port,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Navigate the browser to a URL. Returns the page title after loading.")]
    async fn navigate(
        &self,
        Parameters(NavigateParams { url }): Parameters<NavigateParams>,
    ) -> Result<CallToolResult, McpError> {
        cdp::execute(&self.conn, commands::navigate(&url))
            .await
            .map_err(|e| McpError::internal_error(format!("Navigate failed: {e}"), None))?;

        // Wait a moment for the page to settle, then get the title
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

        let title_result = cdp::execute(&self.conn, commands::evaluate("document.title"))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to get title: {e}"), None))?;

        let title = title_result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)");

        let url_result = cdp::execute(&self.conn, commands::evaluate("window.location.href"))
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
        let result = cdp::execute(&self.conn, commands::screenshot(None, "png"))
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
        let result = cdp::execute(&self.conn, commands::evaluate("document.body.innerText"))
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

    #[tool(description = "Execute JavaScript in the page context and return the result.")]
    async fn evaluate_js(
        &self,
        Parameters(EvaluateJsParams { expression }): Parameters<EvaluateJsParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = cdp::execute(&self.conn, commands::evaluate(&expression))
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
        // Get the element's center coordinates via JS
        let js = format!(
            r#"(() => {{
                const el = document.querySelector({sel});
                if (!el) return null;
                const rect = el.getBoundingClientRect();
                return {{ x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }};
            }})()"#,
            sel = serde_json::to_string(&selector).unwrap()
        );

        let result = cdp::execute(&self.conn, commands::evaluate(&js))
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to find element: {e}"), None))?;

        let coords = result
            .get("result")
            .and_then(|r| r.get("value"))
            .ok_or_else(|| {
                McpError::invalid_params(format!("Element not found: {selector}"), None)
            })?;

        if coords.is_null() {
            return Err(McpError::invalid_params(
                format!("Element not found: {selector}"),
                None,
            ));
        }

        let x = coords.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = coords.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);

        cdp::execute_sequence(&self.conn, commands::click(x, y))
            .await
            .map_err(|e| McpError::internal_error(format!("Click failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Clicked '{selector}' at ({x:.0}, {y:.0})"
        ))]))
    }

    #[tool(description = "Type text into an element on the page. Focuses the element first, then types character by character.")]
    async fn type_text(
        &self,
        Parameters(TypeTextParams { selector, text }): Parameters<TypeTextParams>,
    ) -> Result<CallToolResult, McpError> {
        // Focus the element first
        let js = format!(
            r#"(() => {{
                const el = document.querySelector({sel});
                if (!el) return false;
                el.focus();
                return true;
            }})()"#,
            sel = serde_json::to_string(&selector).unwrap()
        );

        let result = cdp::execute(&self.conn, commands::evaluate(&js))
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

        // Type each character
        cdp::execute_sequence(&self.conn, commands::type_text(&text))
            .await
            .map_err(|e| McpError::internal_error(format!("Type failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Typed {len} characters into '{selector}'",
            len = text.len()
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

            let result = cdp::execute(&self.conn, commands::evaluate(&js))
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

        cdp::execute(&self.conn, commands::scroll(scroll_x, scroll_y))
            .await
            .map_err(|e| McpError::internal_error(format!("Scroll failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Scrolled by ({scroll_x:.0}, {scroll_y:.0})"
        ))]))
    }

    #[tool(description = "Navigate back in browser history.")]
    async fn back(&self) -> Result<CallToolResult, McpError> {
        cdp::execute(&self.conn, commands::go_back())
            .await
            .map_err(|e| McpError::internal_error(format!("Back failed: {e}"), None))?;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        Ok(CallToolResult::success(vec![Content::text(
            "Navigated back".to_owned(),
        )]))
    }

    #[tool(description = "Navigate forward in browser history.")]
    async fn forward(&self) -> Result<CallToolResult, McpError> {
        cdp::execute(&self.conn, commands::go_forward())
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

        let result = cdp::execute(&self.conn, commands::evaluate(&js))
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

        let result = cdp::execute(&self.conn, commands::evaluate(&js))
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

        let result = cdp::execute(&self.conn, commands::evaluate(js))
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
