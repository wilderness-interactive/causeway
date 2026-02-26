use serde_json::{Value, json};

/// Navigate to a URL. Returns ("Page.navigate", params).
pub fn navigate(url: &str) -> (&'static str, Value) {
    ("Page.navigate", json!({ "url": url }))
}

/// Capture a screenshot as base64 PNG. Returns ("Page.captureScreenshot", params).
pub fn screenshot(quality: Option<u8>, format: &str) -> (&'static str, Value) {
    let mut params = json!({ "format": format });
    if let Some(q) = quality {
        params["quality"] = json!(q);
    }
    ("Page.captureScreenshot", params)
}

/// Evaluate a JavaScript expression. Returns ("Runtime.evaluate", params).
pub fn evaluate(expression: &str) -> (&'static str, Value) {
    (
        "Runtime.evaluate",
        json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": true,
        }),
    )
}

/// Get the root DOM document. Returns ("DOM.getDocument", params).
pub fn get_document() -> (&'static str, Value) {
    ("DOM.getDocument", json!({}))
}

/// Query a selector within a node. Returns ("DOM.querySelector", params).
pub fn query_selector(node_id: i64, selector: &str) -> (&'static str, Value) {
    (
        "DOM.querySelector",
        json!({ "nodeId": node_id, "selector": selector }),
    )
}

/// Get the box model (position/size) of a node. Returns ("DOM.getBoxModel", params).
pub fn get_box_model(node_id: i64) -> (&'static str, Value) {
    ("DOM.getBoxModel", json!({ "nodeId": node_id }))
}

/// Dispatch a mouse event. Returns ("Input.dispatchMouseEvent", params).
pub fn mouse_event(event_type: &str, x: f64, y: f64, button: &str, click_count: u32) -> (&'static str, Value) {
    (
        "Input.dispatchMouseEvent",
        json!({
            "type": event_type,
            "x": x,
            "y": y,
            "button": button,
            "clickCount": click_count,
        }),
    )
}

/// Build a full click sequence (press + release) at coordinates.
pub fn click(x: f64, y: f64) -> Vec<(&'static str, Value)> {
    vec![
        mouse_event("mousePressed", x, y, "left", 1),
        mouse_event("mouseReleased", x, y, "left", 1),
    ]
}

/// Dispatch a key event. Returns ("Input.dispatchKeyEvent", params).
pub fn key_event(event_type: &str, text: &str) -> (&'static str, Value) {
    (
        "Input.dispatchKeyEvent",
        json!({
            "type": event_type,
            "text": text,
        }),
    )
}

/// Build key events for typing a string (char + keyUp per character).
pub fn type_text(text: &str) -> Vec<(&'static str, Value)> {
    text.chars()
        .flat_map(|c| {
            let s = c.to_string();
            vec![
                key_event("char", &s),
            ]
        })
        .collect()
}

/// Navigate back in history.
pub fn go_back() -> (&'static str, Value) {
    // Use JS since CDP doesn't have a direct "back" command
    evaluate("window.history.back()")
}

/// Navigate forward in history.
pub fn go_forward() -> (&'static str, Value) {
    evaluate("window.history.forward()")
}

/// Scroll the page by pixels.
pub fn scroll(x: f64, y: f64) -> (&'static str, Value) {
    evaluate(&format!("window.scrollBy({x}, {y})"))
}

/// Enable required CDP domains.
pub fn enable_page() -> (&'static str, Value) {
    ("Page.enable", json!({}))
}

pub fn enable_dom() -> (&'static str, Value) {
    ("DOM.enable", json!({}))
}

pub fn enable_runtime() -> (&'static str, Value) {
    ("Runtime.enable", json!({}))
}
