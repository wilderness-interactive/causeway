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
        mouse_event("mouseMoved", x, y, "none", 0),
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

/// Get browser navigation history. Returns { currentIndex, entries: [{ id, url, title }] }.
pub fn get_navigation_history() -> (&'static str, Value) {
    ("Page.getNavigationHistory", json!({}))
}

/// Navigate to a specific history entry by its CDP entry ID.
pub fn navigate_to_history_entry(entry_id: i64) -> (&'static str, Value) {
    ("Page.navigateToHistoryEntry", json!({ "entryId": entry_id }))
}

/// Scroll the page by pixels.
pub fn scroll(x: f64, y: f64) -> (&'static str, Value) {
    evaluate(&format!("window.scrollBy({x}, {y})"))
}

/// Evaluate an expression, returning the remote object reference (not the value).
/// Use when you need a nodeId for DOM operations.
pub fn evaluate_ref(expression: &str) -> (&'static str, Value) {
    (
        "Runtime.evaluate",
        json!({
            "expression": expression,
            "returnByValue": false,
        }),
    )
}

/// Set files on a file input element via its Runtime objectId. Bypasses the OS file picker entirely.
pub fn set_file_input_files(object_id: &str, files: &[String]) -> (&'static str, Value) {
    ("DOM.setFileInputFiles", json!({ "objectId": object_id, "files": files }))
}

/// Set a cookie. url is used to infer domain/path if domain is not provided.
pub fn set_cookie(name: &str, value: &str, url: Option<&str>, domain: Option<&str>, path: Option<&str>) -> (&'static str, Value) {
    let mut params = json!({ "name": name, "value": value });
    if let Some(u) = url    { params["url"]    = json!(u); }
    if let Some(d) = domain { params["domain"] = json!(d); }
    if let Some(p) = path   { params["path"]   = json!(p); }
    ("Network.setCookie", params)
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

pub fn enable_network() -> (&'static str, Value) {
    ("Network.enable", json!({}))
}

/// Double-click at coordinates (press+release count=1, then press+release count=2).
pub fn double_click(x: f64, y: f64) -> Vec<(&'static str, Value)> {
    vec![
        mouse_event("mousePressed", x, y, "left", 1),
        mouse_event("mouseReleased", x, y, "left", 1),
        mouse_event("mousePressed", x, y, "left", 2),
        mouse_event("mouseReleased", x, y, "left", 2),
    ]
}

/// Drag from (from_x, from_y) to (to_x, to_y) with n intermediate mouse-move steps.
pub fn drag(from_x: f64, from_y: f64, to_x: f64, to_y: f64, steps: u32) -> Vec<(&'static str, Value)> {
    let mut cmds = Vec::new();
    cmds.push(("Input.dispatchMouseEvent", json!({
        "type": "mousePressed", "x": from_x, "y": from_y,
        "button": "left", "buttons": 1, "clickCount": 1,
    })));
    let n = steps.max(1);
    for i in 1..=n {
        let t = i as f64 / n as f64;
        cmds.push(("Input.dispatchMouseEvent", json!({
            "type": "mouseMoved",
            "x": from_x + (to_x - from_x) * t,
            "y": from_y + (to_y - from_y) * t,
            "button": "left", "buttons": 1,
        })));
    }
    cmds.push(("Input.dispatchMouseEvent", json!({
        "type": "mouseReleased", "x": to_x, "y": to_y,
        "button": "left", "buttons": 0, "clickCount": 1,
    })));
    cmds
}

/// Set browser viewport dimensions.
pub fn set_viewport(width: u32, height: u32) -> (&'static str, Value) {
    ("Emulation.setDeviceMetricsOverride", json!({
        "width": width, "height": height,
        "deviceScaleFactor": 1, "mobile": false,
    }))
}

/// Enable the Accessibility CDP domain.
pub fn enable_accessibility() -> (&'static str, Value) {
    ("Accessibility.enable", json!({}))
}

/// Get the full accessibility tree of the current page.
pub fn get_full_ax_tree() -> (&'static str, Value) {
    ("Accessibility.getFullAXTree", json!({}))
}

/// Handle a JavaScript dialog (alert/confirm/prompt/beforeunload).
pub fn handle_dialog(accept: bool, prompt_text: Option<&str>) -> (&'static str, Value) {
    let mut params = json!({ "accept": accept });
    if let Some(text) = prompt_text {
        params["promptText"] = json!(text);
    }
    ("Page.handleJavaScriptDialog", params)
}

/// Dispatch keyDown+keyUp with modifier keys. modifiers: Alt=1, Ctrl=2, Meta=4, Shift=8.
pub fn key_chord(key: &str, modifiers: u32) -> Vec<(&'static str, Value)> {
    // Map key to physical code and virtual key code (same lookup as press_key)
    let (code, vk) = if key.len() == 1 && key.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        // Letter keys: code = "KeyA", vk = 65..90 (uppercase ASCII)
        let upper = key.to_uppercase();
        (format!("Key{upper}"), upper.chars().next().unwrap() as u32)
    } else if key.len() == 1 && key.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        // Digit keys: code = "Digit0", vk = 48..57
        (format!("Digit{key}"), key.chars().next().unwrap() as u32)
    } else {
        // Special keys — reuse the same VK lookup as press_key
        let vk = match key {
            "Enter" => 13, "Tab" => 9, "Escape" => 27, "Backspace" => 8,
            "Delete" => 46, "Space" => 32, "Home" => 36, "End" => 35,
            "PageUp" => 33, "PageDown" => 34,
            "ArrowUp" => 38, "ArrowDown" => 40, "ArrowLeft" => 37, "ArrowRight" => 39,
            "F1" => 112, "F2" => 113, "F3" => 114, "F4" => 115,
            "F5" => 116, "F6" => 117, "F7" => 118, "F8" => 119,
            "F9" => 120, "F10" => 121, "F11" => 122, "F12" => 123,
            _ => 0,
        };
        (key.to_owned(), vk)
    };
    vec![
        ("Input.dispatchKeyEvent", json!({
            "type": "keyDown", "modifiers": modifiers,
            "key": key, "code": code,
            "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk,
        })),
        ("Input.dispatchKeyEvent", json!({
            "type": "keyUp", "modifiers": modifiers,
            "key": key, "code": code,
            "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk,
        })),
    ]
}

/// Print current page as PDF (base64 encoded).
pub fn print_to_pdf() -> (&'static str, Value) {
    ("Page.printToPDF", json!({
        "printBackground": true,
        "preferCSSPageSize": true,
    }))
}

/// Enable the Performance CDP domain.
pub fn enable_performance() -> (&'static str, Value) {
    ("Performance.enable", json!({}))
}

/// Get browser performance metrics.
pub fn get_metrics() -> (&'static str, Value) {
    ("Performance.getMetrics", json!({}))
}

/// Clear browser HTTP cache.
pub fn clear_browser_cache() -> (&'static str, Value) {
    ("Network.clearBrowserCache", json!({}))
}

/// Clear storage data for an origin.
pub fn clear_data_for_origin(origin: &str, storage_types: &str) -> (&'static str, Value) {
    ("Storage.clearDataForOrigin", json!({
        "origin": origin,
        "storageTypes": storage_types,
    }))
}

/// Override the user agent string.
pub fn set_user_agent(user_agent: &str) -> (&'static str, Value) {
    ("Emulation.setUserAgentOverride", json!({
        "userAgent": user_agent,
    }))
}

/// Enable or disable touch event emulation.
pub fn set_touch_emulation(enabled: bool) -> (&'static str, Value) {
    ("Emulation.setTouchEmulationEnabled", json!({
        "enabled": enabled,
    }))
}

/// Full device metrics emulation (viewport + scale + mobile flag).
pub fn emulate_device_metrics(width: u32, height: u32, scale_factor: f64, mobile: bool) -> (&'static str, Value) {
    ("Emulation.setDeviceMetricsOverride", json!({
        "width": width,
        "height": height,
        "deviceScaleFactor": scale_factor,
        "mobile": mobile,
    }))
}

/// Clear all device emulation overrides.
pub fn clear_device_override() -> (&'static str, Value) {
    ("Emulation.clearDeviceMetricsOverride", json!({}))
}

/// Stealth: inject script before any page JS to hide CDP detection signals.
/// Runs via Page.addScriptToEvaluateOnNewDocument so it executes before page scripts.
pub fn add_stealth_script() -> (&'static str, Value) {
    ("Page.addScriptToEvaluateOnNewDocument", json!({
        "source": r#"
            // Hide navigator.webdriver
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });

            // Restore navigator.permissions.query for 'notifications'
            const origQuery = window.Notification && Notification.permission;
            if (navigator.permissions) {
                const origPermQuery = navigator.permissions.query;
                navigator.permissions.query = (params) => {
                    if (params.name === 'notifications') {
                        return Promise.resolve({ state: origQuery || 'prompt' });
                    }
                    return origPermQuery.call(navigator.permissions, params);
                };
            }

            // Mask chrome.runtime to look like a normal browser (not headless)
            if (!window.chrome) window.chrome = {};
            if (!window.chrome.runtime) window.chrome.runtime = {};

            // Remove CDP-specific properties from Error stack traces
            const origGetOwnPropertyDescriptor = Object.getOwnPropertyDescriptor;
            // Prevent detection of overridden properties
            const nativeToString = Function.prototype.toString;
            const overrides = new Map();
            const handler = {
                apply: function(target, thisArg, args) {
                    if (overrides.has(thisArg)) return overrides.get(thisArg);
                    return nativeToString.call(thisArg);
                }
            };
            Function.prototype.toString = new Proxy(nativeToString, handler);
            overrides.set(Function.prototype.toString, 'function toString() { [native code] }');
        "#
    }))
}
