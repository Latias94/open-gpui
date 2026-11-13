use super::{BROWSER_TIMEOUT, CdpClient, POLL_INTERVAL};
use serde_json::{Value, json};
use std::{thread, time::Instant};

const STATE: &str = "({...globalThis.__OPEN_GPUI_CLIPBOARD__, url: location.href})";
const WRITE_IMAGE: &str = r#"
(async () => {
    const canvas = document.createElement('canvas');
    canvas.width = 2;
    canvas.height = 2;
    const context = canvas.getContext('2d');
    context.fillStyle = '#2468ac';
    context.fillRect(0, 0, 2, 2);
    const blob = await new Promise(resolve => canvas.toBlob(resolve, 'image/png'));
    await navigator.clipboard.write([new ClipboardItem({'image/png': blob})]);
    const stored = (await navigator.clipboard.read())[0];
    const actual = await stored.getType('image/png');
    const bytes = new Uint8Array(await actual.arrayBuffer());
    let digest = 2166136261;
    for (const byte of bytes) digest = Math.imul(digest ^ byte, 16777619) >>> 0;
    const decoded = await createImageBitmap(actual);
    context.drawImage(decoded, 0, 0);
    const pixel = Array.from(context.getImageData(0, 0, 1, 1).data);
    decoded.close();
    return {digest, bytes: bytes.length, pixel};
})()
"#;

pub(super) fn qualify(cdp: &mut CdpClient, url: &str) -> Result<(), String> {
    let origin = url.split('?').next().unwrap_or(url).trim_end_matches('/');
    grant(cdp, origin)?;
    let clipboard_url = format!("{url}&clipboard=1");
    let navigation = cdp.call("Page.navigate", json!({"url": clipboard_url}))?;
    if let Some(error) = navigation.get("errorText").and_then(Value::as_str) {
        return Err(format!("clipboard fixture navigation failed: {error}"));
    }
    wait(cdp, "registered handler", |state| {
        state["url"] == clipboard_url
            && number(state, "paints") > 0
            && state["installedTarget"] == 0
            && number(state, "acceptedGeneration") > 0
    })?;
    cdp.evaluate("document.querySelector('textarea').focus(); true")?;
    let previous_text =
        cdp.evaluate("navigator.clipboard.readText().then(text => ({text})).catch(() => ({}))")?;
    let outcome = qualify_contents(cdp, origin);
    let restoration = (|| {
        grant(cdp, origin)?;
        if let Some(text) = previous_text.get("text").and_then(Value::as_str) {
            cdp.evaluate(&format!(
                "navigator.clipboard.writeText({}).then(() => true)",
                json!(text)
            ))?;
        }
        Ok::<(), String>(())
    })();
    outcome?;
    restoration?;
    println!(
        "web clipboard passed: real permission-granted text/image reads and trusted DOM paste, actual denial, consumed-key suppression, delivery-time focus and retirement; delayed Blob and API-absence controls are explicit test controls; only prior plain text is restored"
    );
    Ok(())
}

fn qualify_contents(cdp: &mut CdpClient, origin: &str) -> Result<(), String> {
    let paste_modifiers = if cfg!(target_os = "macos") { 4 } else { 2 };
    cdp.evaluate("navigator.clipboard.writeText('Async 文本').then(() => true)")?;
    key(cdp, "r", 0)?;
    wait(cdp, "real async text", |state| {
        state["readStatus"] == "item"
            && state["text0"] == "Async 文本"
            && state["activationAtRead"] == true
            && number(state, "asyncCompletions") == 1
    })?;

    cdp.evaluate("navigator.clipboard.writeText(' DOM 贴').then(() => true)")?;
    key(cdp, "v", paste_modifiers)?;
    wait(cdp, "trusted single DOM text paste", |state| {
        state["text0"] == "Async 文本 DOM 贴" && number(state, "pastes0") == 2
    })?;
    key(cdp, "k", 0)?;
    key(cdp, "v", paste_modifiers)?;
    let state = cdp.evaluate(STATE)?;
    if state["text0"] != "Async 文本 DOM 贴" || number(&state, "pastes0") != 2 {
        return Err(format!("consumed paste shortcut inserted again: {state}"));
    }
    key(cdp, "k", 0)?;

    key(cdp, "p", 0)?;
    let written = cdp.evaluate("navigator.clipboard.readText()")?;
    if written != "Written by GPUI 文本" {
        return Err(format!(
            "framework clipboard write did not reach browser: {written}"
        ));
    }

    cdp.call(
        "Browser.setPermission",
        json!({"permission": {"name": "clipboard-read"}, "setting": "denied", "origin": origin}),
    )?;
    key(cdp, "r", 0)?;
    wait(cdp, "real permission denial", |state| {
        state["readStatus"] == "denied" && number(state, "asyncCompletions") == 2
    })?;
    grant(cdp, origin)?;

    let image = cdp.evaluate(WRITE_IMAGE)?;
    if image["pixel"] != json!([36, 104, 172, 255]) || number(&image, "bytes") == 0 {
        return Err(format!(
            "native clipboard PNG fixture changed pixels: {image}"
        ));
    }
    key(cdp, "r", 0)?;
    let before_dom_image = wait(cdp, "real asynchronous image read", |state| {
        state["readStatus"] == "item"
            && number(state, "asyncCompletions") == 3
            && number(state, "imageBytes0") > 0
            && number(state, "pastes0") == 3
            && state["entries0"] == "Png"
            && state["imageDigest0"] == image["digest"]
    })?;
    key(cdp, "v", paste_modifiers)?;
    wait(cdp, "real DOM image paste", |state| {
        number(state, "pastes0") == 4
            && number(state, "imageBytes0") > number(&before_dom_image, "imageBytes0")
            && state["entries0"] == "Png"
            && state["imageDigest0"] == image["digest"]
    })?;

    cdp.evaluate(
        r#"(() => {
            const clipboard = navigator.clipboard;
            Object.defineProperty(navigator, 'clipboard', {configurable: true, value: undefined});
            globalThis.__restoreClipboard = () => {
                Object.defineProperty(navigator, 'clipboard', {configurable: true, value: clipboard});
            };
            return true;
        })()"#,
    )?;
    key(cdp, "r", 0)?;
    wait(cdp, "explicit API absence control", |state| {
        state["readStatus"] == "unavailable"
    })?;
    cdp.evaluate("__restoreClipboard(); true")?;

    cdp.evaluate(
        r#"(() => {
            const read = navigator.clipboard.read.bind(navigator.clipboard);
            navigator.clipboard.read = () => Promise.reject(new DOMException('fixture data', 'DataError'));
            globalThis.__restoreRead = () => {navigator.clipboard.read = read;};
            return true;
        })()"#,
    )?;
    key(cdp, "r", 0)?;
    wait(cdp, "explicit unsupported rejection control", |state| {
        state["readStatus"] == "unsupported"
    })?;
    cdp.evaluate("__restoreRead(); true")?;

    let before_cancel = cdp.evaluate(STATE)?;
    cdp.evaluate(
        r#"(() => {
            const read = navigator.clipboard.read.bind(navigator.clipboard);
            globalThis.__clipboardReadReady = false;
            navigator.clipboard.read = () => {
                const request = read();
                return new Promise((resolve, reject) => request.then(value => {
                    globalThis.__clipboardReadReady = true;
                    globalThis.__completeClipboardRead = () => resolve(value);
                }, reject));
            };
            globalThis.__restoreRead = () => {navigator.clipboard.read = read;};
            return true;
        })()"#,
    )?;
    key(cdp, "r", 0)?;
    let started = Instant::now();
    while cdp.evaluate("__clipboardReadReady")? != true {
        if started.elapsed() >= BROWSER_TIMEOUT {
            return Err("real clipboard request did not reach the controlled completion".into());
        }
        thread::sleep(POLL_INTERVAL);
    }
    key(cdp, "c", 0)?;
    cdp.evaluate("__restoreRead(); __completeClipboardRead(); true")?;
    let cancelled = cdp.evaluate(
        "new Promise(resolve => setTimeout(() => resolve(globalThis.__OPEN_GPUI_CLIPBOARD__), 100))",
    )?;
    if cancelled["asyncCompletions"] != before_cancel["asyncCompletions"]
        || cancelled["pastes0"] != before_cancel["pastes0"]
    {
        return Err(format!("cancelled owned read still delivered: {cancelled}"));
    }

    delay_blobs(cdp)?;
    key(cdp, "v", paste_modifiers)?;
    wait_for_delayed_blob(cdp)?;
    let before_switch = cdp.evaluate(STATE)?;
    key(cdp, "s", 0)?;
    wait(cdp, "replacement focus accepted", |state| {
        state["active"] == 1
            && state["installedTarget"] == 1
            && number(state, "acceptedGeneration") > number(&before_switch, "acceptedGeneration")
    })?;
    cdp.evaluate("__releaseClipboardBlobs(); true")?;
    wait(cdp, "image reached current handler", |state| {
        number(state, "pastes1") == 1
            && number(state, "imageBytes1") > 0
            && state["pastes0"] == before_switch["pastes0"]
            && state["entries1"] == "Png"
            && state["imageDigest1"] == image["digest"]
    })?;
    let before_close = cdp.evaluate(STATE)?;

    delay_blobs(cdp)?;
    key(cdp, "v", paste_modifiers)?;
    wait_for_delayed_blob(cdp)?;
    key(cdp, "x", 0)?;
    cdp.evaluate("__releaseClipboardBlobs(); true")?;
    let closed = cdp.evaluate(
        "new Promise(resolve => setTimeout(() => resolve(globalThis.__OPEN_GPUI_CLIPBOARD__), 100))",
    )?;
    if closed["closed"] != true || closed["pastes1"] != before_close["pastes1"] {
        return Err(format!("late image reached retired window: {closed}"));
    }
    Ok(())
}

fn grant(cdp: &mut CdpClient, origin: &str) -> Result<(), String> {
    cdp.call(
        "Browser.grantPermissions",
        json!({"origin": origin, "permissions": ["clipboardReadWrite", "clipboardSanitizedWrite"]}),
    )?;
    Ok(())
}

fn key(cdp: &mut CdpClient, key: &str, modifiers: u8) -> Result<(), String> {
    let code = format!("Key{}", key.to_ascii_uppercase());
    let virtual_key = key.as_bytes()[0].to_ascii_uppercase();
    cdp.editing_key(key, &code, u32::from(virtual_key), modifiers)
}

fn number(state: &Value, field: &str) -> u64 {
    state[field].as_u64().unwrap_or(0)
}

fn wait(
    cdp: &mut CdpClient,
    label: &str,
    predicate: impl Fn(&Value) -> bool,
) -> Result<Value, String> {
    let started = Instant::now();
    let mut last = Value::Null;
    while started.elapsed() < BROWSER_TIMEOUT {
        last = cdp.evaluate(STATE)?;
        if predicate(&last) {
            return Ok(last);
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(format!("timed out waiting for clipboard {label}: {last}"))
}

fn delay_blobs(cdp: &mut CdpClient) -> Result<(), String> {
    cdp.evaluate(
        r#"(() => {
            const original = Blob.prototype.arrayBuffer;
            const pending = [];
            globalThis.__clipboardBlobWaits = pending;
            Blob.prototype.arrayBuffer = function () {
                const blob = this;
                return new Promise((resolve, reject) => pending.push(() => original.call(blob).then(resolve, reject)));
            };
            globalThis.__releaseClipboardBlobs = () => {
                Blob.prototype.arrayBuffer = original;
                for (const resume of pending.splice(0)) resume();
            };
            return true;
        })()"#,
    )?;
    Ok(())
}

fn wait_for_delayed_blob(cdp: &mut CdpClient) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < BROWSER_TIMEOUT {
        if cdp
            .evaluate("__clipboardBlobWaits.length")?
            .as_u64()
            .unwrap_or(0)
            > 0
        {
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err("trusted image paste did not capture a File before awaiting".into())
}
