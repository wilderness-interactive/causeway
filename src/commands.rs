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

/// Dispatch a special key (Enter, Tab, Escape, etc.) via keyDown + keyUp.
pub fn press_key(key: &str) -> Vec<(&'static str, Value)> {
    let (key_code, code, windows_vk) = match key {
        "Enter" => (13, "Enter", 13),
        "Tab" => (9, "Tab", 9),
        "Escape" => (27, "Escape", 27),
        "Backspace" => (8, "Backspace", 8),
        "Delete" => (46, "Delete", 46),
        "ArrowUp" => (38, "ArrowUp", 38),
        "ArrowDown" => (40, "ArrowDown", 40),
        "ArrowLeft" => (37, "ArrowLeft", 37),
        "ArrowRight" => (39, "ArrowRight", 39),
        "Home" => (36, "Home", 36),
        "End" => (35, "End", 35),
        "PageUp" => (33, "PageUp", 33),
        "PageDown" => (34, "PageDown", 34),
        "Space" => (32, "Space", 32),
        _ => (0, key, 0),
    };

    vec![
        (
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown",
                "key": key,
                "code": code,
                "windowsVirtualKeyCode": windows_vk,
                "nativeVirtualKeyCode": key_code,
            }),
        ),
        (
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": key,
                "code": code,
                "windowsVirtualKeyCode": windows_vk,
                "nativeVirtualKeyCode": key_code,
            }),
        ),
    ]
}

/// Build a hover sequence (mouseMoved to coordinates).
pub fn hover(x: f64, y: f64) -> (&'static str, Value) {
    mouse_event("mouseMoved", x, y, "none", 0)
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
