use super::{BROWSER_TIMEOUT, CdpClient, POLL_INTERVAL, navigate_case, set_viewport};
use serde_json::{Value, json};
use std::{thread, time::Instant};

const EMOJI: [&str; 10] = [
    "😀",
    "🕸",
    "🕸\u{fe0f}",
    "🕸\u{fe0e}",
    "❤️",
    "👍🏽",
    "🇯🇵",
    "1️⃣",
    "👩‍💻",
    "👨‍👩‍👧‍👦",
];
const CJK: [&str; 6] = ["中", "文", "が", "か\u{3099}", "각", "각"];
const MIXED_INPUT: &str = "A中文 が か\u{3099} 각 각 🕸 🕸\u{fe0f} 🕸\u{fe0e} 🕸 😀 ❤️ 👍🏽 🇯🇵 1️⃣ 👩‍💻 👨‍👩‍👧‍👦Z";

const INSTALL_SPY: &str = r#"
(() => {
    const spy = {
        installed: false, restored: false, failure: null, overflow: false,
        calls: [], limit: 2048,
        trustedInputs: 0, untrustedInputs: 0, trustedKeys: 0, untrustedKeys: 0,
    };
    globalThis.__OPEN_GPUI_FONT_FALLBACK_SPY__ = spy;
    const prototype = globalThis.OffscreenCanvasRenderingContext2D?.prototype;
    if (!prototype) {
        spy.failure = 'OffscreenCanvasRenderingContext2D is unavailable';
        return;
    }
    const originals = [];
    const painted = new WeakMap();
    const record = (context, entry) => {
        if (spy.calls.length >= spy.limit) { spy.overflow = true; return; }
        spy.calls.push({
            ...entry, font: context.font,
            willReadFrequently: context.getContextAttributes().willReadFrequently,
            canvasWidth: context.canvas.width, canvasHeight: context.canvas.height,
        });
    };
    for (const name of ['measureText', 'fillText', 'getImageData']) {
        const original = prototype[name];
        const wrapped = function(...args) {
            const result = Reflect.apply(original, this, args);
            if (spy.calls.length >= spy.limit) { spy.overflow = true; return result; }
            if (name === 'getImageData') {
                const last = painted.get(this);
                if (last) {
                    const bytes = Math.min(result.data.length, 4 * 1024 * 1024);
                    let alphaPixels = 0;
                    for (let index = 3; index < bytes; index += 4) {
                        if (result.data[index] !== 0) alphaPixels++;
                    }
                    record(this, {
                        type: name, text: last.text, alphaPixels,
                        rasterWidth: result.width, rasterHeight: result.height,
                        scanTruncated: bytes !== result.data.length,
                    });
                }
            } else {
                const text = String(args[0]);
                if (name === 'fillText') painted.set(this, {text});
                record(this, {
                    type: name, text,
                    advance: name === 'measureText' ? result.width : null,
                });
            }
            return result;
        };
        originals.push([name, original, wrapped]);
        prototype[name] = wrapped;
    }
    const observe = event => {
        if (!(event.target instanceof HTMLTextAreaElement)) return;
        const key = event.type === 'keydown'
            ? (event.isTrusted ? 'trustedKeys' : 'untrustedKeys')
            : (event.isTrusted ? 'trustedInputs' : 'untrustedInputs');
        spy[key]++;
    };
    document.addEventListener('input', observe, true);
    document.addEventListener('keydown', observe, true);
    spy.restore = () => {
        for (const [name, original, wrapped] of originals) {
            if (prototype[name] === wrapped) prototype[name] = original;
        }
        document.removeEventListener('input', observe, true);
        document.removeEventListener('keydown', observe, true);
        spy.restored = true;
    };
    spy.installed = true;
})();
"#;

const STATE: &str = r#"
(async () => {
    await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('font fallback frames timed out')), 2000);
        requestAnimationFrame(() => requestAnimationFrame(() => { clearTimeout(timer); resolve(); }));
    });
    const spy = globalThis.__OPEN_GPUI_FONT_FALLBACK_SPY__;
    const input = document.querySelector('textarea');
    return {
        url: location.href,
        fixture: globalThis.__OPEN_GPUI_WEB_FONT_FALLBACK__ ?? null,
        inputModel: globalThis.__OPEN_GPUI_WEB_TEXT_INPUT_MODEL__ ?? null,
        input: input ? {
            value: input.value, selectionStart: input.selectionStart, selectionEnd: input.selectionEnd,
            focused: input === document.activeElement,
        } : null,
        spy: spy ? {
            installed: spy.installed, restored: spy.restored, failure: spy.failure,
            overflow: spy.overflow, calls: spy.calls,
            trustedInputs: spy.trustedInputs, untrustedInputs: spy.untrustedInputs,
            trustedKeys: spy.trustedKeys, untrustedKeys: spy.untrustedKeys,
        } : null,
        diagnostics: globalThis.__OPEN_GPUI_WEB_SMOKE_DIAGNOSTICS__ ?? [],
    };
})()
"#;

pub(super) fn qualify(cdp: &mut CdpClient, url: &str) -> Result<(), String> {
    let registration = cdp.call(
        "Page.addScriptToEvaluateOnNewDocument",
        json!({"source": INSTALL_SPY}),
    )?;
    let identifier = registration["identifier"].as_str().ok_or_else(|| {
        format!("font fallback spy returned no script identifier: {registration}")
    })?;
    let result = qualify_cases(cdp, url);
    let restoration = cdp.evaluate(
        "(() => { const spy = globalThis.__OPEN_GPUI_FONT_FALLBACK_SPY__; spy?.restore?.(); return spy?.restored ?? true; })()",
    );
    let removal = cdp.call(
        "Page.removeScriptToEvaluateOnNewDocument",
        json!({"identifier": identifier}),
    );
    if let Err(error) = result {
        return Err(format!(
            "{error}; Canvas method restoration: {restoration:?}; init-script removal: {removal:?}"
        ));
    }
    if restoration? != true {
        return Err("font fallback spy did not restore the current document's methods".into());
    }
    removal?;
    println!(
        "web font fallback passed: stock Inter Variable/JetBrains Mono, default/disabled/emoji/CJK policies, whole emoji and text-default spider sequences, public raw/window layout agreement, accepted-frame atlas paint, bounded Canvas bitmap readback, warm redraw reuse, size/real italic/mono faces, two real font installations and residual missing reports, trusted text insertion and grapheme deletion; no OS IME, display-pixel or performance qualification"
    );
    Ok(())
}

fn qualify_cases(cdp: &mut CdpClient, url: &str) -> Result<(), String> {
    set_viewport(cdp, 800, 600)?;
    let bundled = open(cdp, &format!("{url}&font_fallback_case=1"))?;
    if bundled["fixture"]["model"]["policyQuery"] != "default"
        || bundled["fixture"]["model"]["policy"] != "Emoji"
    {
        return Err(format!(
            "default policy was not left to WebPlatformOptions: {bundled}"
        ));
    }
    check_bundled(&bundled)?;
    let emoji = command(cdp, 1, "emoji")?;
    check_repeated(&emoji, "😀", true, true)?;
    if count(&emoji, "fillText", "😀") != 1 || count(&emoji, "measureText", "😀") != 1 {
        return Err(format!(
            "repeated emoji did not share a measurement and atlas raster: {emoji}"
        ));
    }
    redraw(cdp, &emoji)?;
    let spider = command(cdp, 2, "spider")?;
    check_repeated(&spider, "🕸", true, false)?;
    if !(1..=4).contains(&count(&spider, "fillText", "🕸"))
        || count(&spider, "measureText", "🕸") != 1
    {
        return Err(format!(
            "spider cache exceeded one measurement and bounded subpixel rasters: {spider}"
        ));
    }
    redraw(cdp, &spider)?;
    let mixed = command(cdp, 3, "mixed")?;
    check_policy(&mixed, true, false)?;
    let residual = wait(cdp, "residual default-policy reports", |state| {
        reported(state, 0, "中", "Proportional")
            && reported(state, 0, "か\u{3099}", "Proportional")
            && reported(state, 0, "ش", "Monospace")
            && state["fixture"]["model"]["callbackReentries"]
                .as_u64()
                .unwrap_or(0)
                > 0
    })?;
    check_policy(&residual, true, false)?;

    let larger = command(cdp, 4, "size")?;
    if !calls(&larger).iter().any(|call| {
        call["type"] == "fillText"
            && call["text"] == "😀"
            && call["font"]
                .as_str()
                .is_some_and(|font| font.contains("36px"))
    }) {
        return Err(format!(
            "different text size reused the old raster: {larger}"
        ));
    }
    let measurement = calls(&larger).iter().find(|call| {
        call["type"] == "measureText"
            && call["text"] == "😀"
            && call["font"]
                .as_str()
                .is_some_and(|font| font.contains("36px"))
    });
    let measured_advance = measurement
        .and_then(|call| call["advance"].as_f64())
        .ok_or_else(|| format!("36px fallback has no actual measurement: {larger}"))?;
    let positioned_advance = larger["fixture"]["rows"][0]["glyphs"][1]["x"]
        .as_f64()
        .zip(larger["fixture"]["rows"][0]["glyphs"][0]["x"].as_f64())
        .map(|(next, first)| next - first)
        .ok_or_else(|| format!("36px fallback has no positioned glyph pair: {larger}"))?;
    if (measured_advance - positioned_advance).abs() > 0.001 {
        return Err(format!(
            "36px positioning reused an old measurement: {larger}"
        ));
    }
    redraw(cdp, &larger)?;
    let italic = command(cdp, 5, "italic")?;
    let regular_row = &bundled["fixture"]["rows"][0];
    let italic_row = &italic["fixture"]["rows"][0];
    if italic_row["directFont"] == regular_row["directFont"]
        || italic_row["mRaster"].is_null()
        || italic_row["mRaster"] == regular_row["mRaster"]
        || !calls(&italic).iter().any(|call| {
            call["type"] == "fillText"
                && call["text"] == "🕸"
                && call["font"]
                    .as_str()
                    .is_some_and(|font| font.contains("italic"))
        })
    {
        return Err(format!(
            "italic request did not select and rasterize a real different face: {italic}"
        ));
    }
    redraw(cdp, &italic)?;
    let face = command(cdp, 6, "face")?;
    if face["fixture"]["rows"][0]["directFont"] == regular_row["directFont"]
        || !face["fixture"]["rows"][1]["directFont"].is_null()
        || count(&face, "measureText", "😀") <= count(&italic, "measureText", "😀")
    {
        return Err(format!(
            "mono face did not get its own cache or Lilex was already installed: {face}"
        ));
    }
    redraw(cdp, &face)?;
    qualify_installation(cdp, &face, 7, "regular-installed", 1)?;
    let installed = wait(cdp, "Regular installation reports", |state| {
        reported(state, 1, "ش", "Monospace")
    })?;
    let bold = qualify_installation(cdp, &installed, 8, "bold-installed", 2)?;
    let rows = &bold["fixture"]["rows"];
    if rows[1]["directFont"] == rows[2]["directFont"]
        || rows[1]["mRaster"] == rows[2]["mRaster"]
        || rows[2]["mRaster"] == installed["fixture"]["rows"][2]["mRaster"]
    {
        return Err(format!(
            "Bold installation did not replace the actual selected face: {bold}"
        ));
    }
    wait(cdp, "Bold installation reports", |state| {
        reported(state, 2, "ش", "Monospace")
    })?;
    redraw(cdp, &bold)?;

    for (policy, emoji_allowed, cjk_allowed) in [
        ("disabled", false, false),
        ("emoji", true, false),
        ("cjk", true, true),
    ] {
        let state = open(
            cdp,
            &format!("{url}&font_fallback_case=1&font_fallback={policy}"),
        )?;
        let expected_policy = match policy {
            "disabled" => "Disabled",
            "emoji" => "Emoji",
            "cjk" => "EmojiAndCjk",
            _ => {
                return Err(format!(
                    "unexpected font fallback qualification policy: {policy}"
                ));
            }
        };
        if state["fixture"]["model"]["policy"] != expected_policy {
            return Err(format!(
                "platform did not receive {expected_policy}: {state}"
            ));
        }
        check_bundled(&state)?;
        check_repeated(
            &command(cdp, 1, "emoji")?,
            "😀",
            emoji_allowed,
            emoji_allowed,
        )?;
        check_repeated(&command(cdp, 2, "spider")?, "🕸", emoji_allowed, false)?;
        let mixed = command(cdp, 3, "mixed")?;
        check_policy(&mixed, emoji_allowed, cjk_allowed)?;
        let residual = wait(cdp, "policy residual reports", |state| {
            reported(state, 0, "ش", "Monospace")
        })?;
        check_policy(&residual, emoji_allowed, cjk_allowed)?;
        if !emoji_allowed && !calls(&mixed).is_empty() {
            return Err(format!(
                "disabled Canvas policy still invoked Canvas: {mixed}"
            ));
        }
    }
    qualify_input(cdp, url)
}

fn qualify_installation(
    cdp: &mut CdpClient,
    before: &Value,
    key: u32,
    phase: &str,
    installation: u32,
) -> Result<Value, String> {
    let after = command(cdp, key, phase)?;
    let rows = &after["fixture"]["rows"];
    if after["fixture"]["model"]["installations"] != installation
        || rows[1]["directFont"].is_null()
        || rows[1]["directFont"] != rows[1]["resolvedFont"]
        || rows[2]["directFont"] != rows[2]["resolvedFont"]
        || count(&after, "measureText", "😀") <= count(before, "measureText", "😀")
        || (installation == 1
            && rows[1]["resolvedFont"] == before["fixture"]["rows"][1]["resolvedFont"])
    {
        return Err(format!(
            "font installation did not invalidate descriptor/layout/Canvas measurement caches: before={before}; after={after}"
        ));
    }
    Ok(after)
}

fn check_bundled(state: &Value) -> Result<(), String> {
    let rows = state["fixture"]["rows"]
        .as_array()
        .ok_or("bundled fixture has no rows")?;
    if rows.len() != 2 || !calls(state).is_empty() {
        return Err(format!(
            "bundled stock text invoked Canvas or lost a consumer: {state}"
        ));
    }
    for (row, family) in rows.iter().zip(["Inter Variable", "JetBrains Mono"]) {
        if row["family"] != family
            || row["directFont"].is_null()
            || row["directFont"] != row["resolvedFont"]
            || row["glyphs"].as_array().is_none_or(|glyphs| {
                glyphs.is_empty()
                    || glyphs
                        .iter()
                        .any(|glyph| glyph["font"] != row["directFont"])
            })
        {
            return Err(format!("stock bundled face was not retained: {state}"));
        }
    }
    Ok(())
}

fn check_repeated(state: &Value, text: &str, fallback: bool, color: bool) -> Result<(), String> {
    let row = &state["fixture"]["rows"][0];
    let glyphs = row["glyphs"]
        .as_array()
        .ok_or("repeated fixture has no glyphs")?;
    if glyphs.len() != 3
        || glyphs.iter().enumerate().any(|(index, glyph)| {
            glyph["index"] != index * text.len()
                || glyph["font"] != glyphs[0]["font"]
                || glyph["id"] != glyphs[0]["id"]
                || (fallback && (glyph["font"] == row["resolvedFont"] || glyph["color"] != color))
        })
        || (fallback && count(state, "getImageData", text) == 0)
        || (!fallback && count(state, "fillText", text) != 0)
    {
        return Err(format!(
            "repeated grapheme did not preserve source/font/glyph/presentation/cache identity: {state}"
        ));
    }
    Ok(())
}

fn check_policy(state: &Value, emoji: bool, cjk: bool) -> Result<(), String> {
    for (texts, allowed) in [(EMOJI.as_slice(), emoji), (CJK.as_slice(), cjk)] {
        for text in texts {
            if (count(state, "fillText", text) > 0) != allowed {
                return Err(format!(
                    "Canvas policy mismatch for whole grapheme {text:?}: {state}"
                ));
            }
        }
    }
    if calls(state)
        .iter()
        .any(|call| matches!(call["text"].as_str(), Some("ش" | "か\u{309a}" | "ᄀ")))
    {
        return Err(format!(
            "unsupported contextual or incomplete text reached Canvas: {state}"
        ));
    }
    if state["fixture"]["model"]["missing"]
        .as_array()
        .is_some_and(|reports| {
            reports.iter().any(|report| {
                let text = report["text"].as_str().unwrap_or("");
                (emoji && EMOJI.contains(&text)) || (cjk && CJK.contains(&text))
            })
        })
    {
        return Err(format!(
            "successful Canvas fallback still emitted a missing report: {state}"
        ));
    }
    Ok(())
}

fn calls(state: &Value) -> &[Value] {
    state["spy"]["calls"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn count(state: &Value, kind: &str, text: &str) -> usize {
    calls(state)
        .iter()
        .filter(|call| call["type"] == kind && call["text"] == text)
        .count()
}

fn reported(state: &Value, installation: u32, text: &str, class: &str) -> bool {
    state["fixture"]["model"]["missing"]
        .as_array()
        .is_some_and(|reports| {
            reports.iter().any(|report| {
                report["installation"] == installation
                    && report["text"] == text
                    && report["fontClass"] == class
            })
        })
}

fn accepted(state: &Value) -> bool {
    let fixture = &state["fixture"];
    fixture["ready"] == true
        && fixture["observation"] == "accepted-app-model"
        && fixture["acceptedGeneration"].as_u64().unwrap_or(0) > 0
        && fixture["rows"].as_array().is_some_and(|rows| {
            !rows.is_empty()
                && rows
                    .iter()
                    .all(|row| row["rawMatchesShaped"] == true && !row["mRaster"].is_null())
        })
}

fn open(cdp: &mut CdpClient, url: &str) -> Result<Value, String> {
    navigate_case(cdp, url)?;
    wait(cdp, "accepted bundled fixture", |state| {
        state["url"] == url && accepted(state)
    })
}

fn command(cdp: &mut CdpClient, number: u32, phase: &str) -> Result<Value, String> {
    let before = cdp.evaluate(STATE)?;
    let name = format!("F{number}");
    key(cdp, &name, &name, 111 + number, 0)?;
    wait(cdp, phase, |state| {
        accepted(state)
            && state["fixture"]["model"]["phase"] == phase
            && state["fixture"]["model"]["commandRevision"].as_u64()
                > before["fixture"]["model"]["commandRevision"].as_u64()
            && state["fixture"]["acceptedGeneration"].as_u64()
                > before["fixture"]["acceptedGeneration"].as_u64()
    })
}

fn redraw(cdp: &mut CdpClient, before: &Value) -> Result<(), String> {
    let phase = before["fixture"]["model"]["phase"]
        .as_str()
        .ok_or("redraw has no phase")?;
    let after = command(cdp, 9, phase)?;
    if calls(&after) != calls(before) || after["fixture"]["rows"] != before["fixture"]["rows"] {
        return Err(format!(
            "warm redraw changed glyph identity or remeasured/rerasterized: before={before}; after={after}"
        ));
    }
    Ok(())
}

fn qualify_input(cdp: &mut CdpClient, url: &str) -> Result<(), String> {
    navigate_case(cdp, &format!("{url}&text_input=1"))?;
    let initial = wait(cdp, "focused public text consumer", |state| {
        state["inputModel"]["ready"] == true && state["input"]["focused"] == true
    })?;
    let bundled = replace_input(cdp, "Hello ©")?;
    if calls(&initial) != calls(&bundled) {
        return Err(format!("bundled input used Canvas: {bundled}"));
    }
    let emoji = replace_input(cdp, "😀😀😀")?;
    if count(&emoji, "fillText", "😀") != 1 {
        return Err(format!(
            "trusted repeated emoji did not share one atlas raster: {emoji}"
        ));
    }
    caret_cycle(cdp, "😀😀😀", &emoji)?;
    let spider = replace_input(cdp, "🕸🕸🕸")?;
    if !(1..=4).contains(&count(&spider, "fillText", "🕸")) {
        return Err(format!(
            "trusted text-default emoji did not use bounded atlas rasters: {spider}"
        ));
    }
    caret_cycle(cdp, "🕸🕸🕸", &spider)?;
    let mixed = replace_input(cdp, MIXED_INPUT)?;
    check_policy(&mixed, true, false)?;
    key(cdp, "Backspace", "Backspace", 8, 0)?;
    let without_z = MIXED_INPUT
        .strip_suffix('Z')
        .ok_or("input sample has no final Z")?;
    input(cdp, without_z)?;
    key(cdp, "Backspace", "Backspace", 8, 0)?;
    input(
        cdp,
        without_z
            .strip_suffix("👨‍👩‍👧‍👦")
            .ok_or("input sample has no final family")?,
    )?;
    let before = cdp.evaluate(STATE)?;
    let unsupported = replace_input(cdp, "ش")?;
    if calls(&unsupported) != calls(&before)
        || unsupported["spy"]["trustedInputs"].as_u64().unwrap_or(0) == 0
        || unsupported["spy"]["trustedKeys"].as_u64().unwrap_or(0) == 0
        || unsupported["spy"]["untrustedInputs"] != 0
        || unsupported["spy"]["untrustedKeys"] != 0
    {
        return Err(format!(
            "unsupported text used Canvas or edits lacked trusted browser events: {unsupported}"
        ));
    }
    Ok(())
}

fn caret_cycle(cdp: &mut CdpClient, text: &str, before: &Value) -> Result<(), String> {
    let end = text.encode_utf16().count();
    for (name, code, offset) in [("ArrowLeft", 37, end - 2), ("ArrowRight", 39, end)] {
        let generation = cdp.evaluate(STATE)?["inputModel"]["acceptedGeneration"].as_u64();
        key(cdp, name, name, code, 0)?;
        wait(cdp, "accepted trusted caret movement", |state| {
            state["inputModel"]["acceptedGeneration"].as_u64() > generation
                && state["inputModel"]["fields"][0]["editableText"] == text
                && state["input"]["value"] == text
                && state["input"]["selectionStart"] == offset
                && state["input"]["selectionEnd"] == offset
                && state["inputModel"]["fields"][0]["selectionUtf16"]
                    == json!({"start": offset + 2, "end": offset + 2})
        })?;
        let after = input(cdp, text)?;
        if calls(&after) != calls(before) {
            return Err(format!(
                "trusted caret movement remeasured or rerasterized emoji: {after}"
            ));
        }
    }
    Ok(())
}

fn replace_input(cdp: &mut CdpClient, text: &str) -> Result<Value, String> {
    let before = cdp.evaluate(STATE)?;
    let previous = before["inputModel"]["fields"][0]["editableText"]
        .as_str()
        .ok_or("replacement has no accepted input model")?
        .to_owned();
    key(
        cdp,
        "a",
        "KeyA",
        65,
        if cfg!(target_os = "macos") { 4 } else { 2 },
    )?;
    let end = previous.encode_utf16().count();
    wait(
        cdp,
        "accepted trusted select-all before replacement",
        |state| {
            state["inputModel"]["ready"] == true
                && state["inputModel"]["fields"][0]["editableText"] == previous
                && state["inputModel"]["fields"][0]["selectionUtf16"]
                    == json!({"start": 2, "end": end + 2})
                && state["input"]["value"] == previous
                && state["input"]["selectionStart"] == 0
                && state["input"]["selectionEnd"] == end
        },
    )?;
    cdp.call("Input.insertText", json!({"text": text}))?;
    input(cdp, text)
}

fn input(cdp: &mut CdpClient, text: &str) -> Result<Value, String> {
    wait(
        cdp,
        "accepted Rust text and actual textarea agreement",
        |state| {
            let model = &state["inputModel"];
            let field = &model["fields"][0];
            let expected = format!("A⟦{text}⟧");
            model["ready"] == true
                && model["observation"] == "accepted-app-model"
                && model["acceptedGeneration"].as_u64().unwrap_or(0) > 0
                && model["focusedField"] == 0
                && field["focused"] == true
                && field["focusRendered"] == true
                && field["text"] == expected
                && field["paintedText"] == expected
                && field["editableText"] == text
                && field["markedRangeUtf16"].is_null()
                && state["input"]["focused"] == true
                && state["input"]["value"] == text
                && state["input"]["selectionStart"]
                    .as_u64()
                    .is_some_and(|index| field["selectionUtf16"]["start"] == index + 2)
                && state["input"]["selectionEnd"]
                    .as_u64()
                    .is_some_and(|index| field["selectionUtf16"]["end"] == index + 2)
        },
    )
}

fn key(
    cdp: &mut CdpClient,
    key: &str,
    code: &str,
    virtual_key: u32,
    modifiers: u8,
) -> Result<(), String> {
    for kind in ["keyDown", "keyUp"] {
        cdp.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": kind, "key": key, "code": code, "modifiers": modifiers,
                "windowsVirtualKeyCode": virtual_key, "nativeVirtualKeyCode": virtual_key,
            }),
        )?;
    }
    Ok(())
}

fn wait(
    cdp: &mut CdpClient,
    label: &str,
    predicate: impl Fn(&Value) -> bool,
) -> Result<Value, String> {
    let started = Instant::now();
    let mut last = Value::Null;
    while started.elapsed() < BROWSER_TIMEOUT {
        match cdp.evaluate(STATE) {
            Ok(state) => {
                let browser_error = state["diagnostics"].as_array().is_some_and(|entries| {
                    entries.iter().any(|entry| {
                        matches!(
                            entry["level"].as_str(),
                            Some("error" | "console.error" | "unhandledrejection")
                        ) || entry["message"].as_str().is_some_and(|message| {
                            message.contains("Canvas font fallback failed")
                                || message.contains("willReadFrequently")
                        })
                    })
                });
                let bad_canvas_call = calls(&state).iter().any(|call| {
                    call["willReadFrequently"] != true
                        || (call["type"] == "measureText"
                            && call["advance"]
                                .as_f64()
                                .is_none_or(|width| !width.is_finite() || width <= 0.0))
                        || (call["type"] == "getImageData"
                            && (call["alphaPixels"].as_u64().unwrap_or(0) == 0
                                || call["scanTruncated"] != false))
                });
                if browser_error
                    || bad_canvas_call
                    || state["spy"]["overflow"] == true
                    || state["spy"]["installed"] != true
                    || !state["spy"]["failure"].is_null()
                    || state["fixture"]["model"]["reportOverflow"] == true
                    || !state["fixture"]["model"]["failure"].is_null()
                {
                    return Err(format!(
                        "font fallback {label} reported a browser/fixture failure: {state}"
                    ));
                }
                if predicate(&state) {
                    return Ok(state);
                }
                last = state;
            }
            Err(error) => last = json!({"evaluation_error": error}),
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(format!(
        "timed out waiting for font fallback {label}: {last}"
    ))
}
