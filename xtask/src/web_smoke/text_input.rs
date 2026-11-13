use super::{BROWSER_TIMEOUT, CdpClient, POLL_INTERVAL, navigate_case, number_field, set_viewport};
use serde_json::{Value, json};
use std::{ops::Range, thread, time::Instant};

const SEEDS: [&str; 2] = ["A😀e\u{301}中\nsecond", "β🦀two"];
const PROGRAMMATIC: &str = "Programmatic 😀\nselection 中";
const STATE: &str = r#"
(async () => {
    await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('text input animation frames timed out')), 2000);
        requestAnimationFrame(() => requestAnimationFrame(() => { clearTimeout(timer); resolve(); }));
    });
    const dom = globalThis.__OPEN_GPUI_WEB_TEXT_INPUT_DOM__;
    return {
        model: globalThis.__OPEN_GPUI_WEB_TEXT_INPUT_MODEL__ ?? null,
        dom: dom?.current ?? null,
        events: dom ? {
            focusIns: dom.focusIns, focusOuts: dom.focusOuts, overflow: dom.overflow,
            syntheticStarts: dom.events.filter(e => e.kind === 'compositionstart' && !e.trusted).length,
            syntheticEnds: dom.events.filter(e => e.kind === 'compositionend' && !e.trusted).length,
            trustedEdits: dom.events.filter(e => e.kind === 'input' && e.trusted).length,
            untrustedEdits: dom.events.filter(e => ['input', 'beforeinput'].includes(e.kind) && !e.trusted).length,
        } : null,
        controls: globalThis.__textInputQualification ?? null,
        coarsePointer: matchMedia('(pointer: coarse)').matches,
        diagnostics: globalThis.__OPEN_GPUI_WEB_SMOKE_DIAGNOSTICS__ ?? [],
    };
})()
"#;
const INSTALL_CONTROLS: &str = r#"
(() => {
    const input = document.querySelector('textarea');
    if (!input) throw new Error('text input fixture has no textarea');
    const control = {
        trustedKeys: 0, untrustedKeys: 0, attributeMutations: 0,
        blurEnds: 0, commitOnBlur: null,
        evidence: 'Synthetic DOM composition controls; not OS IME qualification',
    };
    globalThis.__textInputQualification = control;
    document.addEventListener('keydown', event => {
        if (event.target instanceof HTMLTextAreaElement) {
            control[event.isTrusted ? 'trustedKeys' : 'untrustedKeys']++;
        }
    }, true);
    new MutationObserver(records => { control.attributeMutations += records.length; }).observe(input, {
        attributes: true,
        attributeFilter: ['autocorrect', 'autocapitalize', 'spellcheck', 'autocomplete', 'enterkeyhint'],
    });
    input.addEventListener('blur', () => {
        const data = control.commitOnBlur;
        if (data === null) return;
        control.commitOnBlur = null;
        control.blurEnds++;
        input.dispatchEvent(new CompositionEvent('compositionend', {bubbles: true, data}));
    });
    return true;
})()
"#;

pub(super) fn qualify(cdp: &mut CdpClient, url: &str) -> Result<(), String> {
    let fixture_url = format!("{url}&text_input=1");
    set_viewport(cdp, 800, 600)?;
    navigate_case(cdp, &fixture_url)?;
    cdp.evaluate(INSTALL_CONTROLS)?;
    qualify_edits(cdp)?;
    qualify_configuration(cdp)?;
    qualify_fields(cdp)?;
    qualify_composition(cdp)?;
    qualify_focus_and_resize(cdp)?;

    cdp.call(
        "Emulation.setTouchEmulationEnabled",
        json!({"enabled": true, "maxTouchPoints": 2}),
    )?;
    let result = qualify_keyboard_reentry(cdp, &fixture_url);
    let reset_touch = cdp.call(
        "Emulation.setTouchEmulationEnabled",
        json!({"enabled": false}),
    );
    let reset_viewport = set_viewport(cdp, 800, 600);
    if let Err(error) = result {
        return Err(format!(
            "{error}; touch reset: {reset_touch:?}; viewport reset: {reset_viewport:?}"
        ));
    }
    reset_touch?;
    reset_viewport?;
    println!(
        "web text input passed: trusted CDP editing and UTF-16 selection, bounded editable text, accepted Rust model and textarea agreement, public content/selection/configuration notifications, independent fields, external focus recovery and viewport snapshots; synthetic DOM composition commit/cancel/duplicate and synchronous blur controls do not qualify OS IME or mobile keyboard hardware"
    );
    Ok(())
}

fn qualify_edits(cdp: &mut CdpClient) -> Result<(), String> {
    let end = SEEDS[0].encode_utf16().count();
    edited(cdp, "initial accepted field", 0, SEEDS[0], end..end)?;
    let (home, code, virtual_key, modifiers) = if cfg!(target_os = "macos") {
        ("ArrowUp", "ArrowUp", 38, 4)
    } else {
        ("Home", "Home", 36, 2)
    };
    key(cdp, home, code, virtual_key, modifiers)?;
    edited(cdp, "trusted document start", 0, SEEDS[0], 0..0)?;
    key(cdp, "ArrowRight", "ArrowRight", 39, 0)?;
    edited(cdp, "caret before surrogate pair", 0, SEEDS[0], 1..1)?;
    key(cdp, "ArrowRight", "ArrowRight", 39, 8)?;
    let selected = edited(cdp, "partial UTF-16 selection", 0, SEEDS[0], 1..3)?;
    if selected["model"]["fields"][0]["selectedText"] != "😀" {
        return Err(format!(
            "partial selection did not select the surrogate pair: {selected}"
        ));
    }
    insert(cdp, "Z")?;
    edited(cdp, "partial replacement", 0, "AZe\u{301}中\nsecond", 2..2)?;
    key(cdp, "Backspace", "Backspace", 8, 0)?;
    edited(cdp, "trusted backspace", 0, "Ae\u{301}中\nsecond", 1..1)?;
    insert(cdp, "\n")?;
    edited(
        cdp,
        "trusted newline insertion",
        0,
        "A\ne\u{301}中\nsecond",
        2..2,
    )?;
    select_all(cdp)?;
    let body = "A\ne\u{301}中\nsecond";
    edited(
        cdp,
        "selection excludes protected text",
        0,
        body,
        0..body.encode_utf16().count(),
    )?;
    key(cdp, "Backspace", "Backspace", 8, 0)?;
    edited(cdp, "delete entire editable range", 0, "", 0..0)?;
    key(cdp, "Backspace", "Backspace", 8, 0)?;
    key(cdp, "Delete", "Delete", 46, 0)?;
    edited(cdp, "protected boundaries survive deletion", 0, "", 0..0)?;
    insert(cdp, "edge😀")?;
    let before = edited(cdp, "bounded insertion", 0, "edge😀", 6..6)?;
    command(cdp, 4)?;
    let end = PROGRAMMATIC.encode_utf16().count();
    let content = edited(
        cdp,
        "F4 public content notification",
        0,
        PROGRAMMATIC,
        end..end,
    )?;
    let previous = &before["model"]["fields"][0]["counts"];
    let counts = &content["model"]["fields"][0]["counts"];
    if counts["programmaticEdits"].as_u64() != previous["programmaticEdits"].as_u64().map(|n| n + 1)
        || counts["contentNotifications"].as_u64() <= previous["contentNotifications"].as_u64()
    {
        return Err(format!(
            "F4 did not notify through the public consumer: {content}"
        ));
    }
    command(cdp, 5)?;
    let selection = edited(
        cdp,
        "F5 public selection notification",
        0,
        PROGRAMMATIC,
        1..3,
    )?;
    let updated = &selection["model"]["fields"][0];
    if updated["selectedText"] != "ro"
        || updated["counts"]["contentNotifications"] != counts["contentNotifications"]
        || updated["counts"]["selectionNotifications"].as_u64()
            <= counts["selectionNotifications"].as_u64()
    {
        return Err(format!(
            "F5 did not preserve content while publishing selection: {selection}"
        ));
    }
    if selection["controls"]["trustedKeys"].as_u64().unwrap_or(0) == 0
        || selection["controls"]["untrustedKeys"] != 0
        || selection["events"]["trustedEdits"].as_u64().unwrap_or(0) == 0
        || selection["events"]["untrustedEdits"] != 0
    {
        return Err(format!(
            "ordinary edits did not use trusted browser input: {selection}"
        ));
    }
    Ok(())
}

fn qualify_configuration(cdp: &mut CdpClient) -> Result<(), String> {
    let before = edited(cdp, "configuration baseline", 0, PROGRAMMATIC, 1..3)?;
    command(cdp, 6)?;
    let configured = wait(cdp, "F6 configuration publication", |state| {
        field_matches(state, 0, PROGRAMMATIC)
            && state["model"]["fields"][0]["configuration"]
                == json!({
                    "autocorrect": true, "autocapitalize": "Sentences",
                    "suggestions": true, "inputAction": "Search",
                })
            && state["dom"]["mirror"]["autocorrect"] == "on"
            && state["dom"]["mirror"]["autocapitalize"] == "sentences"
            && state["dom"]["mirror"]["spellcheck"] == "true"
            && state["dom"]["mirror"]["enterKeyHint"] == "search"
            && state["controls"]["attributeMutations"].as_u64()
                > before["controls"]["attributeMutations"].as_u64()
    })?;
    retained_dom(&before, &configured)?;
    if configured["model"]["fields"][0]["selectionUtf16"]
        != before["model"]["fields"][0]["selectionUtf16"]
        || configured["model"]["fields"][0]["counts"]["configurationChanges"].as_u64()
            != before["model"]["fields"][0]["counts"]["configurationChanges"]
                .as_u64()
                .map(|n| n + 1)
    {
        return Err(format!(
            "F6 changed selection or missed its configuration callback: {configured}"
        ));
    }
    command(cdp, 7)?;
    let repainted = wait(cdp, "F7 accepted redraw", |state| {
        field_matches(state, 0, PROGRAMMATIC)
            && state["model"]["acceptedGeneration"].as_u64()
                > configured["model"]["acceptedGeneration"].as_u64()
            && state["model"]["renderCalls"].as_u64() > configured["model"]["renderCalls"].as_u64()
    })?;
    retained_dom(&configured, &repainted)?;
    if repainted["controls"]["attributeMutations"] != configured["controls"]["attributeMutations"]
        || repainted["model"]["fields"] != configured["model"]["fields"]
    {
        return Err(format!(
            "ordinary repaint restarted or changed the text session: {repainted}"
        ));
    }
    Ok(())
}

fn qualify_fields(cdp: &mut CdpClient) -> Result<(), String> {
    let before = edited(cdp, "field A retained selection", 0, PROGRAMMATIC, 1..3)?;
    command(cdp, 2)?;
    let end = SEEDS[1].encode_utf16().count();
    let second = edited(cdp, "F2 independent field B", 1, SEEDS[1], end..end)?;
    if second["dom"]["mirror"]["autocapitalize"] != "words"
        || second["dom"]["mirror"]["enterKeyHint"] != "send"
    {
        return Err(format!(
            "field B did not install its own configuration: {second}"
        ));
    }
    select_all(cdp)?;
    edited(cdp, "field B editable selection", 1, SEEDS[1], 0..end)?;
    let body = "B-only 🦀";
    insert(cdp, body)?;
    let end = body.encode_utf16().count();
    edited(cdp, "field B replacement", 1, body, end..end)?;
    command(cdp, 1)?;
    let restored = edited(cdp, "F1 field A recovery", 0, PROGRAMMATIC, 1..3)?;
    for property in [
        "text",
        "selectionUtf16",
        "markedRangeUtf16",
        "configuration",
        "counts",
    ] {
        if restored["model"]["fields"][0][property] != before["model"]["fields"][0][property] {
            return Err(format!("editing B changed A's {property}: {restored}"));
        }
    }
    if restored["model"]["fields"][1]["editableText"] != body {
        return Err(format!(
            "switching back to A lost B's model text: {restored}"
        ));
    }
    Ok(())
}

fn qualify_composition(cdp: &mut CdpClient) -> Result<(), String> {
    command(cdp, 8)?;
    let end = SEEDS[0].encode_utf16().count();
    edited(cdp, "composition reset", 0, SEEDS[0], end..end)?;
    start_composition(cdp, "漢😀")?;
    let body = format!("{}漢😀", SEEDS[0]);
    composition_event(cdp, "compositionend", "漢😀")?;
    let end = body.encode_utf16().count();
    let committed = edited(cdp, "synthetic composition commit", 0, &body, end..end)?;
    composition_event(cdp, "compositionend", "漢😀")?;
    let duplicate = edited(
        cdp,
        "synthetic duplicate composition end",
        0,
        &body,
        end..end,
    )?;
    if duplicate["model"]["fields"][0]["counts"]["replacements"]
        != committed["model"]["fields"][0]["counts"]["replacements"]
    {
        return Err(format!(
            "duplicate composition end performed another replacement: {duplicate}"
        ));
    }
    start_composition(cdp, "取消")?;
    composition_event(cdp, "compositionend", "")?;
    let cancelled = edited(
        cdp,
        "synthetic composition cancellation",
        0,
        &body,
        end..end,
    )?;
    if cancelled["events"]["syntheticStarts"] != 2 || cancelled["events"]["syntheticEnds"] != 3 {
        return Err(format!(
            "synthetic composition controls were not observed: {cancelled}"
        ));
    }
    Ok(())
}

fn qualify_focus_and_resize(cdp: &mut CdpClient) -> Result<(), String> {
    let body = format!("{}漢😀", SEEDS[0]);
    let end = body.encode_utf16().count();
    let before = edited(cdp, "external focus baseline", 0, &body, end..end)?;
    let button = cdp.evaluate(
        r#"(() => {
        document.getElementById('text-input-observations').open = true;
        const button = document.getElementById('text-input-external-focus');
        button.scrollIntoView({block: 'nearest'});
        const rect = button.getBoundingClientRect();
        return {x: rect.x + rect.width / 2, y: rect.y + rect.height / 2};
    })()"#,
    )?;
    cdp.mouse_click(number_field(&button, "x")?, number_field(&button, "y")?)?;
    let external = wait(cdp, "browser external focus", |state| {
        state["dom"]["activeElementId"] == "text-input-external-focus"
            && state["dom"]["mirror"]["focused"] == false
            && state["model"]["windowActive"] == false
    })?;
    if external["model"]["fields"][0]["text"] != before["model"]["fields"][0]["text"] {
        return Err(format!(
            "external DOM focus changed the App model: {external}"
        ));
    }
    cdp.evaluate("document.getElementById('text-input-observations').open = false; true")?;
    let bounds = &before["model"]["fields"][0]["bounds"];
    cdp.mouse_click(
        number_field(bounds, "x")? + 12.0,
        number_field(bounds, "y")? + 12.0,
    )?;
    let recovered = wait(cdp, "trusted canvas focus recovery", |state| {
        field_matches(state, 0, &body) && state["model"]["windowActive"] == true
    })?;
    set_viewport(cdp, 720, 520)?;
    let result = wait(cdp, "accepted viewport resize snapshot", |state| {
        field_matches(state, 0, &body)
            && state["model"]["acceptedGeneration"].as_u64()
                > recovered["model"]["acceptedGeneration"].as_u64()
            && state["model"]["viewport"]["layout"]["width"] == 720
            && state["model"]["viewport"]["layout"]["height"] == 520
            && state["dom"]["layout"]["width"] == 720
            && state["dom"]["layout"]["height"] == 520
            && ["x", "y", "width", "height"].iter().all(|key| {
                state["model"]["viewport"]["visual"][*key] == state["dom"]["visual"][*key]
            })
    });
    let restoration = set_viewport(cdp, 800, 600);
    let resized =
        result.map_err(|error| format!("{error}; viewport restoration: {restoration:?}"))?;
    restoration?;
    retained_dom(&recovered, &resized)?;
    if resized["model"]["fields"][0]["selectionUtf16"]
        != recovered["model"]["fields"][0]["selectionUtf16"]
    {
        return Err(format!("resize changed the accepted selection: {resized}"));
    }
    println!(
        "web text input viewport snapshot: {}",
        resized["model"]["viewport"]
    );
    Ok(())
}

fn qualify_keyboard_reentry(cdp: &mut CdpClient, url: &str) -> Result<(), String> {
    cdp.call(
        "Emulation.setDeviceMetricsOverride",
        json!({
            "width": 800, "height": 600, "deviceScaleFactor": 1,
            "mobile": true, "screenWidth": 800, "screenHeight": 600,
        }),
    )?;
    navigate_case(
        cdp,
        &format!("{url}&text_input_case=touch-keyboard&activation=programmatic-only"),
    )?;
    cdp.evaluate(INSTALL_CONTROLS)?;
    command(cdp, 1)?;
    let baseline = wait(cdp, "touch keyboard request", |state| {
        field_matches(state, 0, SEEDS[0])
            && state["coarsePointer"] == true
            && state["dom"]["mirror"]["inputMode"] == "text"
    })?;
    let mut body = SEEDS[0].to_owned();
    for (index, function_key, text, mode) in [(1, 9, "重入", "none"), (2, 10, "復帰", "text")] {
        start_composition(cdp, text)?;
        cdp.evaluate(
            r#"(() => {
            const field = globalThis.__OPEN_GPUI_WEB_TEXT_INPUT_MODEL__.fields[0];
            if (!field.markedText) throw new Error('blur composition control has no marked text');
            globalThis.__textInputQualification.commitOnBlur = field.markedText;
            return true;
        })()"#,
        )?;
        command(cdp, function_key)?;
        body.push_str(text);
        let state = wait(
            cdp,
            "synthetic blur composition during keyboard operation",
            |state| {
                field_matches(state, 0, &body)
                    && state["controls"]["blurEnds"] == index
                    && state["controls"]["commitOnBlur"].is_null()
                    && state["dom"]["mirror"]["inputMode"] == mode
                    && state["model"]["windowActive"] == true
            },
        )?;
        if state["model"]["viewport"]["layout"] != baseline["model"]["viewport"]["layout"]
            || state["dom"]["mirror"]["nodeId"] != baseline["dom"]["mirror"]["nodeId"]
        {
            return Err(format!(
                "F{function_key} changed layout or replaced the textarea: {state}"
            ));
        }
    }
    command(cdp, 11)?;
    wait(cdp, "irreversible native input quiescence", |state| {
        state["dom"]["mirror"]["disabled"] == true
            && state["dom"]["mirror"]["readOnly"] == true
            && state["dom"]["mirror"]["tabIndex"] == -1
            && state["dom"]["mirror"]["focused"] == false
    })?;
    let focused = cdp.evaluate("(() => { const input = document.querySelector('textarea'); input.focus(); return document.activeElement === input; })()")?;
    if focused != false {
        return Err("the retired textarea regained native focus".to_owned());
    }
    cdp.evaluate("document.getElementById('text-input-observations').open = true; document.getElementById('text-input-external-focus').focus(); true")?;
    key(cdp, "Tab", "Tab", 9, 0)?;
    wait(
        cdp,
        "retired mirror stays outside native Tab navigation",
        |state| {
            state["dom"]["mirror"]["focused"] == false && state["dom"]["mirror"]["disabled"] == true
        },
    )?;
    Ok(())
}

fn field_matches(state: &Value, target: usize, body: &str) -> bool {
    let field = &state["model"]["fields"][target];
    let mirror = &state["dom"]["mirror"];
    let text = format!("{}{}⟧", ["A⟦", "B⟦"][target], body);
    state["model"]["ready"] == true
        && state["model"]["observation"] == "accepted-app-model"
        && state["model"]["acceptedGeneration"].as_u64().unwrap_or(0) > 0
        && state["model"]["fields"]
            .as_array()
            .is_some_and(|fields| fields.len() == 2)
        && state["model"]["focusedField"] == target
        && field["focused"] == true
        && field["focusRendered"] == true
        && field["text"] == text
        && field["paintedText"] == text
        && field["editableText"] == body
        && field["lengthUtf16"] == text.encode_utf16().count()
        && field["editableRangeUtf16"]
            == json!({"start": 2, "end": 2 + body.encode_utf16().count()})
        && field["markedRangeUtf16"].is_null()
        && field["markedText"].is_null()
        && mirror["tagName"] == "TEXTAREA"
        && mirror["focused"] == true
        && mirror["readOnly"] == false
        && mirror["disabled"] == false
        && mirror["tabIndex"] == -1
        && mirror["value"] == body
        && mirror["selectionStart"]
            .as_u64()
            .is_some_and(|n| field["selectionUtf16"]["start"] == n + 2)
        && mirror["selectionEnd"]
            .as_u64()
            .is_some_and(|n| field["selectionUtf16"]["end"] == n + 2)
}

fn edited(
    cdp: &mut CdpClient,
    label: &str,
    target: usize,
    body: &str,
    selection: Range<usize>,
) -> Result<Value, String> {
    wait(cdp, label, |state| {
        field_matches(state, target, body)
            && state["model"]["fields"][target]["selectionUtf16"]
                == json!({"start": selection.start + 2, "end": selection.end + 2})
    })
}

fn retained_dom(before: &Value, after: &Value) -> Result<(), String> {
    if before["dom"]["mirror"]["nodeId"] != after["dom"]["mirror"]["nodeId"]
        || after["dom"]["mirror"]["focused"] != true
        || before["events"]["focusIns"] != after["events"]["focusIns"]
        || before["events"]["focusOuts"] != after["events"]["focusOuts"]
    {
        return Err(format!(
            "text session replaced its DOM node or cycled focus: before={before}; after={after}"
        ));
    }
    Ok(())
}

fn start_composition(cdp: &mut CdpClient, text: &str) -> Result<(), String> {
    let before = cdp.evaluate(STATE)?;
    let field = &before["model"]["fields"][0];
    let body = format!(
        "{}{text}",
        field["editableText"]
            .as_str()
            .ok_or("composition baseline has no text")?
    );
    let start = field["selectionUtf16"]["start"]
        .as_u64()
        .ok_or("composition baseline has no selection")?;
    composition_event(cdp, "compositionstart", "")?;
    composition_event(cdp, "compositionupdate", text)?;
    wait(cdp, "synthetic marked text accepted by Rust", |state| {
        let field = &state["model"]["fields"][0];
        state["model"]["observation"] == "accepted-app-model"
            && state["model"]["ready"] == true
            && field["editableText"] == body
            && field["paintedText"] == field["text"]
            && field["markedText"] == text
            && field["markedRangeUtf16"]
                == json!({"start": start, "end": start + text.encode_utf16().count() as u64})
    })?;
    Ok(())
}

fn composition_event(cdp: &mut CdpClient, kind: &str, text: &str) -> Result<(), String> {
    let dispatch = r#"(kind, data) => {
        const input = document.querySelector('textarea');
        if (document.activeElement !== input) {
            throw new Error('synthetic composition requires focused textarea');
        }
        input.dispatchEvent(new CompositionEvent(kind, {bubbles: true, data}));
        return true;
    }"#;
    cdp.evaluate(&format!("({dispatch})({}, {})", json!(kind), json!(text),))?;
    Ok(())
}

fn insert(cdp: &mut CdpClient, text: &str) -> Result<(), String> {
    cdp.call("Input.insertText", json!({"text": text}))?;
    Ok(())
}

fn select_all(cdp: &mut CdpClient) -> Result<(), String> {
    let modifiers = if cfg!(target_os = "macos") { 4 } else { 2 };
    key(cdp, "a", "KeyA", 65, modifiers)
}

fn command(cdp: &mut CdpClient, number: u32) -> Result<(), String> {
    let name = format!("F{number}");
    key(cdp, &name, &name, 111 + number, 0)
}

fn key(
    cdp: &mut CdpClient,
    key: &str,
    code: &str,
    virtual_key: u32,
    modifiers: u8,
) -> Result<(), String> {
    cdp.editing_key(key, code, virtual_key, modifiers)
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
                if state["events"]["overflow"] == true
                    || state["diagnostics"].as_array().is_some_and(|entries| {
                        entries.iter().any(|entry| {
                            matches!(
                                entry["level"].as_str(),
                                Some("error" | "console.error" | "unhandledrejection")
                            )
                        })
                    })
                {
                    return Err(format!(
                        "text input {label} reported a browser failure: {state}"
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
    Err(format!("timed out waiting for text input {label}: {last}"))
}
