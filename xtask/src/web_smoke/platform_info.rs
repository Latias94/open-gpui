use super::{BROWSER_TIMEOUT, CdpClient, POLL_INTERVAL};
use serde_json::{Value, json};
use std::{thread, time::Instant};

const CDP_CONTROL: u8 = 2;
const CDP_META: u8 = 4;

const STATE: &str = r#"
(() => {
    const canvas = document.querySelector('canvas');
    const rect = canvas?.getBoundingClientRect();
    const getterFailures = [];
    const report = {};
    for (const field of ['userAgent', 'platform', 'maxTouchPoints']) {
        try { report[field] = navigator[field]; }
        catch { report[field] = null; getterFailures.push(field); }
    }
    return {
        url: location.href,
        probe: globalThis.__OPEN_GPUI_PLATFORM_INFO__ ?? null,
        control: globalThis.__OPEN_GPUI_PLATFORM_INFO_CONTROL__ ?? null,
        keys: globalThis.__platformInfoKeys ?? null,
        navigator: report,
        getterFailures,
        inputFocused: document.activeElement === document.querySelector('input'),
        canvas: rect ? {x: rect.x, y: rect.y, width: rect.width, height: rect.height} : null,
    };
})()
"#;

pub(super) fn qualify(cdp: &mut CdpClient, url: &str) -> Result<(), String> {
    let fixture_url = format!("{url}&platform_info=1");
    let native_os = match std::env::consts::OS {
        "macos" => "Mac",
        "windows" => "Windows",
        "linux" => "Linux",
        "freebsd" => "FreeBsd",
        "ios" => "Ios",
        "android" => "Android",
        _ => "Unknown",
    };
    let native = qualify_case(cdp, &fixture_url, "native", native_os, None, true)?;
    let controls = qualify_controls(cdp, &fixture_url);
    let restoration = (|| {
        let restored = open(
            cdp,
            &format!("{fixture_url}&platform_info_case=native-restored"),
        )?;
        check_identity(&restored, native_os)?;
        check_report(&restored, None)?;
        if restored["navigator"] != native["navigator"] {
            return Err(format!("native navigator was not restored: {restored}"));
        }
        Ok::<(), String>(())
    })();
    match (controls, restoration) {
        (Ok(()), Ok(())) => {}
        (Err(error), restoration) => {
            return Err(format!(
                "platform info controls failed: {error}; navigator restoration: {restoration:?}"
            ));
        }
        (Ok(()), Err(error)) => return Err(error),
    }
    println!(
        "web platform identity passed: original browser host, navigator Mac/Windows/Linux/iPhone/iPad and five throwing-getter controls, Web target, public labels, trusted single/chord dispatch for native/Mac/Windows/Linux and cached identity after navigator mutation in every sample; fixture paint callbacks do not prove scanout or pixels"
    );
    Ok(())
}

fn qualify_controls(cdp: &mut CdpClient, url: &str) -> Result<(), String> {
    for (name, os, platform, user_agent, touch_points, failures) in [
        (
            "mac",
            "Mac",
            "MacIntel",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
            0,
            &[] as &[&str],
        ),
        (
            "windows",
            "Windows",
            "Win32",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
            0,
            &[],
        ),
        (
            "linux",
            "Linux",
            "Linux x86_64",
            "Mozilla/5.0 (X11; Linux x86_64)",
            0,
            &[],
        ),
        (
            "iphone",
            "Ios",
            "iPhone",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)",
            5,
            &[],
        ),
        (
            "ipad",
            "Ios",
            "MacIntel",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
            5,
            &[],
        ),
        (
            "ua-throws-mac",
            "Mac",
            "MacIntel",
            "Windows",
            0,
            &["userAgent"],
        ),
        (
            "platform-throws-windows",
            "Windows",
            "MacIntel",
            "Windows",
            5,
            &["platform"],
        ),
        (
            "platform-throws-mac",
            "Mac",
            "MacIntel",
            "Mac",
            5,
            &["platform"],
        ),
        (
            "both-throw",
            "Unknown",
            "Win32",
            "Android",
            5,
            &["userAgent", "platform"],
        ),
        (
            "ua-throws-ipad",
            "Ios",
            "MacIntel",
            "Windows",
            2,
            &["userAgent"],
        ),
    ] {
        let control = json!({
            "name": name,
            "platform": platform,
            "userAgent": user_agent,
            "maxTouchPoints": touch_points,
            "failures": failures,
        });
        let source = format!(
            r#"
            (() => {{
                const sample = {control};
                for (const key of ['platform', 'userAgent', 'maxTouchPoints']) {{
                    Object.defineProperty(navigator, key, {{
                        configurable: true,
                        get() {{
                            if (sample.failures.includes(key)) throw new Error(`navigator.${{key}} extraction failed`);
                            return sample[key];
                        }},
                    }});
                }}
                globalThis.__OPEN_GPUI_PLATFORM_INFO_CONTROL__ = sample;
            }})();
        "#
        );
        let registration = cdp.call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source": source}),
        )?;
        let identifier = registration
            .get("identifier")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!("navigator control returned no script identifier: {registration}")
            })?;
        let result = qualify_case(
            cdp,
            url,
            name,
            os,
            Some(&control),
            matches!(name, "mac" | "windows" | "linux"),
        );
        let removal = cdp.call(
            "Page.removeScriptToEvaluateOnNewDocument",
            json!({"identifier": identifier}),
        );
        if let Err(error) = result {
            return Err(format!(
                "navigator sample {name}: {error}; init-script removal: {removal:?}"
            ));
        }
        removal?;
    }
    Ok(())
}

fn qualify_case(
    cdp: &mut CdpClient,
    url: &str,
    name: &str,
    os: &str,
    control: Option<&Value>,
    full_dispatch: bool,
) -> Result<Value, String> {
    let state = open(cdp, &format!("{url}&platform_info_case={name}"))?;
    check_identity(&state, os)?;
    check_report(&state, control)?;
    let canvas = &state["canvas"];
    let coordinate = |field: &str| {
        canvas[field]
            .as_f64()
            .ok_or_else(|| format!("platform info canvas has no {field}: {state}"))
    };
    cdp.mouse_click(
        coordinate("x")? + coordinate("width")? / 2.0,
        coordinate("y")? + coordinate("height")? / 2.0,
    )?;
    cdp.evaluate(
        r#"(() => {
        const input = document.querySelector('input');
        if (!input) throw new Error('platform info input is missing');
        input.focus();
        const keys = {trusted: 0, untrusted: 0, input: 0, lastKey: ''};
        globalThis.__platformInfoKeys = keys;
        document.addEventListener('keydown', event => {
            keys[event.isTrusted ? 'trusted' : 'untrusted']++;
            if (event.target === input) keys.input++;
            keys.lastKey = event.key;
        }, true);
        return true;
    })()"#,
    )?;
    wait(cdp, "real focused view", |state| {
        state["inputFocused"] == true
            && state["probe"]["focused"] == true
            && state["probe"]["focusClicks"].as_u64().unwrap_or(0) > 0
    })?;
    if !full_dispatch {
        check_cached_identity(cdp, &state, os)?;
        println!(
            "web platform identity sample={name} os={os} target=Web: classification, labels and cached identity passed"
        );
        return Ok(state);
    }
    let secondary = if os == "Mac" { CDP_META } else { CDP_CONTROL };
    let opposite = if secondary == CDP_META {
        CDP_CONTROL
    } else {
        CDP_META
    };
    let mut counts = [0; 4];

    for (modifiers, expected) in [(secondary, true), (opposite, false)] {
        key(cdp, "u", modifiers)?;
        check_unbound(cdp, "u", expected, counts)?;
    }
    key(cdp, "j", opposite)?;
    check_unbound(cdp, "j", false, counts)?;
    key(cdp, "k", opposite)?;
    check_unbound(cdp, "k", false, counts)?;
    key(cdp, "j", opposite)?;
    check_unbound(cdp, "j", false, counts)?;

    key(cdp, "j", secondary)?;
    counts[0] += 1;
    check_actions(cdp, counts)?;
    key(cdp, "k", secondary)?;
    let pending = cdp.evaluate(STATE)?;
    if pending["probe"]["pendingChord"] != true || action_counts(&pending) != counts {
        return Err(format!(
            "chord prefix dispatched early or was not pending: {pending}"
        ));
    }
    key(cdp, "j", secondary)?;
    counts[1] += 1;
    check_actions(cdp, counts)?;

    for (modifiers, index) in [(CDP_META, 2), (CDP_CONTROL, 3)] {
        key(cdp, "y", modifiers)?;
        counts[index] += 1;
        check_actions(cdp, counts)?;
    }
    let completed = cdp.evaluate(STATE)?;
    if completed["keys"]["trusted"] != 10
        || completed["keys"]["untrusted"] != 0
        || completed["keys"]["input"] != 10
    {
        return Err(format!(
            "keys did not take the trusted browser input path: {completed}"
        ));
    }
    check_identity(&completed, os)?;
    check_report(&completed, control)?;
    check_cached_identity(cdp, &completed, os)?;
    println!(
        "web platform identity sample={name} os={os} target=Web: labels, trusted dispatch and cached identity passed"
    );
    Ok(completed)
}

fn check_identity(state: &Value, os: &str) -> Result<(), String> {
    let probe = &state["probe"];
    let (single, chord, command, ctrl) = match os {
        "Mac" => ("⌘J", "⌘K ⌘J", "⌘Y", "^Y"),
        "Windows" => ("ctrl-J", "ctrl-K ctrl-J", "⊞Y", "ctrl-Y"),
        "Linux" | "FreeBsd" => ("ctrl-J", "ctrl-K ctrl-J", "❖Y", "ctrl-Y"),
        _ => ("ctrl-J", "ctrl-K ctrl-J", "cmd-Y", "ctrl-Y"),
    };
    if probe["os"] != os || probe["target"] != "Web" || probe["isMac"] != (os == "Mac") {
        return Err(format!("expected backend identity {os}/Web: {state}"));
    }
    for (field, expected) in [
        ("singleLabel", single),
        ("chordLabel", chord),
        ("commandLabel", command),
        ("controlLabel", ctrl),
        ("parsedSingleLabel", single),
    ] {
        if probe[field] != expected {
            return Err(format!("expected {os} {field}={expected:?}: {state}"));
        }
    }
    Ok(())
}

fn check_report(state: &Value, control: Option<&Value>) -> Result<(), String> {
    if let Some(control) = control {
        if state["control"] != *control {
            return Err(format!(
                "navigator control did not run before app startup: {state}"
            ));
        }
        if state["getterFailures"] != control["failures"] {
            return Err(format!(
                "navigator getters did not throw as controlled: {state}"
            ));
        }
        for field in ["userAgent", "platform", "maxTouchPoints"] {
            let throws = control["failures"]
                .as_array()
                .is_some_and(|failures| failures.iter().any(|failure| failure == field));
            let matches = if throws {
                state["navigator"][field].is_null()
            } else {
                state["navigator"][field] == control[field]
            };
            if !matches {
                return Err(format!(
                    "navigator {field} differs from its explicit control: {state}"
                ));
            }
        }
    } else if !state["control"].is_null() || state["getterFailures"] != json!([]) {
        return Err(format!(
            "original host case retained a navigator control: {state}"
        ));
    }
    Ok(())
}

fn check_cached_identity(cdp: &mut CdpClient, before: &Value, os: &str) -> Result<(), String> {
    let replacement = if os == "Mac" {
        json!({"userAgent": "Windows", "platform": "Win32", "maxTouchPoints": 0})
    } else {
        json!({"userAgent": "Mac", "platform": "MacIntel", "maxTouchPoints": 0})
    };
    let baseline = cdp.evaluate(&format!(
        r#"(() => {{
        const properties = ['userAgent', 'platform', 'maxTouchPoints'];
        const original = properties.map(key => [key, Object.getOwnPropertyDescriptor(navigator, key)]);
        globalThis.__restorePlatformInfoReport = () => {{
            for (const [key, descriptor] of original) {{
                if (descriptor) Object.defineProperty(navigator, key, descriptor);
                else delete navigator[key];
            }}
            delete globalThis.__restorePlatformInfoReport;
        }};
        const report = {replacement};
        for (const key of properties) {{
            Object.defineProperty(navigator, key, {{configurable: true, value: report[key]}});
        }}
        return {{
            renders: globalThis.__OPEN_GPUI_PLATFORM_INFO__.renderCalls,
            keyEvents: globalThis.__OPEN_GPUI_PLATFORM_INFO__.keyEvents,
            trusted: globalThis.__platformInfoKeys.trusted,
        }};
    }})()"#
    ))?;
    let result = (|| {
        key(cdp, "u", if os == "Mac" { CDP_META } else { CDP_CONTROL })?;
        let refreshed = wait(
            cdp,
            "fresh app identity read after navigator mutation",
            |state| {
                state["probe"]["renderCalls"].as_u64().unwrap_or(0)
                    > baseline["renders"].as_u64().unwrap_or(0)
                    && state["probe"]["keyEvents"].as_u64().unwrap_or(0)
                        > baseline["keyEvents"].as_u64().unwrap_or(0)
            },
        )?;
        check_identity(&refreshed, os)?;
        if refreshed["navigator"] != replacement
            || refreshed["getterFailures"] != json!([])
            || refreshed["control"] != before["control"]
            || refreshed["probe"]["lastKey"] != "u"
            || refreshed["probe"]["lastSecondary"] != true
            || refreshed["probe"]["focused"] != true
            || action_counts(&refreshed) != action_counts(before)
            || refreshed["keys"]["trusted"].as_u64()
                != baseline["trusted"].as_u64().map(|count| count + 1)
            || refreshed["keys"]["untrusted"] != 0
            || refreshed["keys"]["input"] != refreshed["keys"]["trusted"]
        {
            return Err(format!(
                "navigator mutation did not preserve cached app identity and input: {refreshed}"
            ));
        }
        Ok(())
    })();
    let restoration = (|| {
        cdp.evaluate("globalThis.__restorePlatformInfoReport(); true")?;
        let restored = cdp.evaluate(STATE)?;
        if restored["navigator"] != before["navigator"]
            || restored["getterFailures"] != before["getterFailures"]
        {
            return Err(format!(
                "navigator descriptors were not restored after mutation: {restored}"
            ));
        }
        Ok::<(), String>(())
    })();
    match (result, restoration) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), restoration) => Err(format!("{error}; report restoration: {restoration:?}")),
        (Ok(()), Err(error)) => Err(error),
    }
}

fn open(cdp: &mut CdpClient, url: &str) -> Result<Value, String> {
    let navigation = cdp.call("Page.navigate", json!({"url": url}))?;
    if let Some(error) = navigation.get("errorText").and_then(Value::as_str) {
        return Err(format!("platform info navigation failed: {error}"));
    }
    wait(cdp, "fresh fixture paint", |state| {
        state["url"] == url
            && state["probe"]["renderCalls"].as_u64().unwrap_or(0) > 0
            && state["probe"]["paintCalls"].as_u64().unwrap_or(0) > 0
            && state["canvas"]["width"].as_f64().unwrap_or(0.0) > 0.0
            && state["canvas"]["height"].as_f64().unwrap_or(0.0) > 0.0
    })
}

fn action_counts(state: &Value) -> [u64; 4] {
    [
        "singleActions",
        "chordActions",
        "commandActions",
        "controlActions",
    ]
    .map(|field| state["probe"][field].as_u64().unwrap_or(u64::MAX))
}

fn check_actions(cdp: &mut CdpClient, expected: [u64; 4]) -> Result<(), String> {
    wait(cdp, "focused action callback", |state| {
        action_counts(state) == expected
            && state["probe"]["actionFocused"] == true
            && state["probe"]["pendingChord"] == false
    })?;
    Ok(())
}

fn check_unbound(
    cdp: &mut CdpClient,
    key: &str,
    secondary: bool,
    expected: [u64; 4],
) -> Result<(), String> {
    let state = cdp.evaluate(STATE)?;
    if action_counts(&state) != expected
        || state["probe"]["lastKey"] != key
        || state["probe"]["lastSecondary"] != secondary
        || state["probe"]["pendingChord"] != false
        || state["probe"]["focused"] != true
    {
        return Err(format!(
            "unbound/negative-control {key} secondary={secondary} changed dispatch: {state}"
        ));
    }
    Ok(())
}

fn key(cdp: &mut CdpClient, key: &str, modifiers: u8) -> Result<(), String> {
    for kind in ["keyDown", "keyUp"] {
        cdp.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": kind,
                "key": key,
                "code": format!("Key{}", key.to_ascii_uppercase()),
                "windowsVirtualKeyCode": key.as_bytes()[0].to_ascii_uppercase(),
                "modifiers": modifiers,
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
        "timed out waiting for platform info {label}: {last}"
    ))
}
