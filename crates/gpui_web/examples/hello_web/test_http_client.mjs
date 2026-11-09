// Build and serve the auxiliary http-client-test Trunk pipeline. Set CHROME
// to an Edge/Chrome executable, then run:
// node test_http_client.mjs http://127.0.0.1:8080
// Requires Node with built-in fetch/WebSocket and a real shared-memory browser.
// Match the standalone lock's event-listener version to the root Cargo.lock;
// its wasm feature union must exclude event-listener/std. Compilation alone
// does not exercise the production synchronization backend or these controls.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { execFile, spawn } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

assert.equal(typeof WebSocket, "function", "Node with a global WebSocket is required");
const origin = new URL(process.argv[2] ?? "http://127.0.0.1:8080").origin;
const windowsBrowser = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
    .flatMap(name => process.env[name] ? [
        path.join(process.env[name], "Microsoft", "Edge", "Application", "msedge.exe"),
        path.join(process.env[name], "Google", "Chrome", "Application", "chrome.exe"),
    ] : [])
    .find(candidate => fs.existsSync(candidate));
const chrome = process.env.CHROME ??
    (process.platform === "darwin"
        ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
        : process.platform === "win32" ? windowsBrowser : "chromium");
assert.ok(chrome, "Set CHROME to a Chrome, Chromium or Edge executable");
const outputRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "target", "http-client-test");
fs.mkdirSync(outputRoot, { recursive: true });
const output = fs.mkdtempSync(path.join(outputRoot, "run-"));
const log = fs.openSync(path.join(output, "chrome.log"), "w");
const browser = spawn(chrome, [
    "--headless=new", "--no-first-run", "--no-default-browser-check",
    "--remote-debugging-address=127.0.0.1", "--remote-debugging-port=0",
    `--user-data-dir=${path.join(output, "profile")}`, "about:blank",
], { windowsHide: true, detached: process.platform !== "win32", stdio: ["ignore", log, log] });
const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
let launchError;
browser.on("error", error => { launchError = error; });
let socket;
let shutdown;
const exited = () => browser.exitCode !== null || browser.signalCode !== null;
const shutdownBrowser = () => shutdown ??= (async () => {
    if (launchError) return;
    assert.ok(browser.pid, "The browser must have an owned process ID before shutdown");
    const groupAlive = () => {
        if (process.platform === "win32") return false;
        try { process.kill(-browser.pid, 0); return true; }
        catch (error) { if (error.code === "ESRCH") return false; throw error; }
    };
    if (exited() && !groupAlive()) return;
    if (process.platform === "win32") {
        try {
            await promisify(execFile)("taskkill.exe", ["/PID", String(browser.pid), "/T", "/F"], {
                windowsHide: true, timeout: 10_000,
            });
        } catch (error) {
            if (!exited()) throw error;
        }
    } else {
        try { process.kill(-browser.pid, "SIGTERM"); }
        catch (error) { if (error.code !== "ESRCH") throw error; }
    }
    const deadline = Date.now() + 5_000;
    while ((!exited() || groupAlive()) && Date.now() < deadline) await delay(20);
    if (groupAlive()) {
        try { process.kill(-browser.pid, "SIGKILL"); }
        catch (error) { if (error.code !== "ESRCH") throw error; }
        const forcedDeadline = Date.now() + 5_000;
        while ((!exited() || groupAlive()) && Date.now() < forcedDeadline) await delay(20);
    }
    assert.ok(exited(), `Owned browser process ${browser.pid} did not exit`);
    assert.ok(!groupAlive(), `Owned browser process group ${browser.pid} did not exit`);
})();
const watchdog = setTimeout(() => {
    console.error(`Browser Fetch test exceeded its external deadline; see ${output}/chrome.log`);
    void shutdownBrowser().catch(error => console.error("Browser cleanup failed:", error)).finally(() => {
        socket?.close();
        process.exitCode = 1;
    });
}, 180_000);
watchdog.unref();

try {
    let port;
    for (let attempt = 0; attempt < 100; attempt++) {
        if (launchError) throw launchError;
        if (browser.exitCode !== null) throw new Error(`Browser exited; see ${output}/chrome.log`);
        const portFile = path.join(output, "profile", "DevToolsActivePort");
        if (fs.existsSync(portFile)) {
            port = fs.readFileSync(portFile, "utf8").split("\n")[0].trim();
            break;
        }
        await delay(100);
    }
    assert.ok(port, "Browser did not publish its debugging port");
    const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`, {
        signal: AbortSignal.timeout(10_000),
    })).json();
    const page = pages.find(page => page.type === "page");
    assert.ok(page, "Browser did not create a page");
    socket = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("Browser WebSocket did not open")), 10_000);
        socket.addEventListener("open", () => { clearTimeout(timer); resolve(); }, { once: true });
        socket.addEventListener("error", error => { clearTimeout(timer); reject(error); }, { once: true });
    });

    let nextId = 0;
    const pending = new Map();
    const errors = [];
    const rejectPending = error => {
        for (const request of pending.values()) {
            clearTimeout(request.timer);
            request.reject(error);
        }
        pending.clear();
    };
    socket.addEventListener("close", () => rejectPending(new Error("Browser WebSocket closed")));
    socket.addEventListener("error", () => rejectPending(new Error("Browser WebSocket failed")));
    socket.addEventListener("message", ({ data }) => {
        const message = JSON.parse(data);
        if (message.id) {
            const request = pending.get(message.id);
            if (!request) return;
            pending.delete(message.id);
            clearTimeout(request.timer);
            if (message.error) request.reject(new Error(JSON.stringify(message.error)));
            else request.resolve(message.result);
        } else if (message.method === "Runtime.exceptionThrown") {
            errors.push(message.params);
            rejectPending(new Error(JSON.stringify(message.params.exceptionDetails)));
        } else if (message.method === "Runtime.consoleAPICalled") {
            const values = message.params.args.map(argument => argument.value ?? argument.description);
            console.log("Browser:", ...values);
        } else if (message.method === "Target.attachedToTarget") {
            void call("Runtime.enable", {}, message.params.sessionId).catch(rejectPending);
        }
    });
    const call = (method, params = {}, sessionId) => new Promise((resolve, reject) => {
        const id = ++nextId;
        // This deadline runs outside the page, including while its main thread is trapped.
        const timer = setTimeout(() => {
            pending.delete(id);
            reject(new Error(`Timed out: ${method}; see ${output}/chrome.log`));
        }, 120_000);
        pending.set(id, { resolve, reject, timer });
        socket.send(JSON.stringify({ id, method, params, sessionId }));
    });
    await call("Runtime.enable");
    await call("Page.enable");
    await call("Target.setAutoAttach", { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
    await call("Emulation.setHardwareConcurrencyOverride", { hardwareConcurrency: 8 });
    const loaded = new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("Page did not load")), 10_000);
        socket.addEventListener("message", function onLoad({ data }) {
            if (JSON.parse(data).method === "Page.loadEventFired") {
                clearTimeout(timer);
                socket.removeEventListener("message", onLoad);
                resolve();
            }
        });
    });
    const navigation = await call("Page.navigate", { url: origin });
    assert.equal(navigation.errorText, undefined, navigation.errorText);
    await loaded;
    const result = await call("Runtime.evaluate", {
        awaitPromise: true, returnByValue: true,
        expression: `(${runBrowserTests.toString()})()`,
    });
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails));
    assert.deepEqual(errors, [], JSON.stringify(errors));
    assert.deepEqual(result.result.value, {
        chunksPerResponse: 32768,
        workerResponses: 8,
        missingWaitAsyncFallback: true,
        controlModes: ["workers", "single-threaded"],
        bodyDropCases: 6,
    });
    console.log("PASS: production browser Fetch streaming, worker contention and cancellation", result.result.value);
} finally {
    clearTimeout(watchdog);
    socket?.close();
    try { await shutdownBrowser(); }
    finally { fs.closeSync(log); }
}

async function runBrowserTests() {
    const check = (condition, message) => { if (!condition) throw new Error(message); };
    const mark = phase => console.info(`OPEN_GPUI_FETCH_STAGE ${phase}`);
    const waitFor = async (predicate, description) => {
        const deadline = performance.now() + 10_000;
        while (!predicate()) {
            check(performance.now() < deadline, `Timed out waiting for ${description}`);
            await new Promise(resolve => setTimeout(resolve, 5));
        }
    };
    check(crossOriginIsolated, "Shared-memory isolation headers are required");
    const fixture = await import("/http-client-test.js");
    const bindings = await fixture.default();
    mark("initialized");
    check(bindings.memory.buffer instanceof SharedArrayBuffer, "Wasm memory must be shared");
    const originalWaitAsync = Object.getOwnPropertyDescriptor(Atomics, "waitAsync");
    check(originalWaitAsync, "This browser must support waitAsync for the worker controls");
    try {
        Object.defineProperty(Atomics, "waitAsync", { ...originalWaitAsync, value: undefined });
        mark("missing-waitAsync fallback");
        await fixture.test_missing_wait_async_fallback();
        mark("missing-waitAsync fallback passed");
    } finally {
        Object.defineProperty(Atomics, "waitAsync", originalWaitAsync);
    }
    let waitForbidden = false;
    try { Atomics.wait(new Int32Array(bindings.memory.buffer), 0, 0, 0); }
    catch (error) { waitForbidden = error instanceof TypeError; }
    check(waitForbidden, "Blocking waits must be forbidden on the browser main thread");

    const originalFetch = globalThis.fetch;
    const stateHook = "__openGpuiFetchWaitForBodyState";
    check(!(stateHook in globalThis), "Fetch test state hook must be unused");
    let responses = 0;
    const streams = [];
    const records = new Map();
    const makeDropStream = record => new ReadableStream({
        pull(controller) {
            record.pulls++;
            if (record.case === "drop-backpressure") {
                controller.enqueue(new Uint8Array([1, 2, 3]));
            } else {
                // Keep reader.read() pending until the Rust body is dropped.
                return new Promise(() => {});
            }
        },
        cancel() {
            record.cancels++;
            record.pullsAtCancel = record.pulls;
        },
    }, { highWaterMark: 0 });

    try {
        globalThis.fetch = () => {
            responses++;
            const record = { pulled: 0, closed: false };
            streams.push(record);
            return Promise.resolve(new Response(new ReadableStream({
                pull(controller) {
                    if (record.pulled === 32768) {
                        record.closed = true;
                        controller.close();
                        return;
                    }
                    const index = record.pulled++;
                    controller.enqueue(new Uint8Array([index % 251, index % 239, index % 227]));
                },
            }, { highWaterMark: 0 })));
        };
        mark("eight-worker stream");
        await fixture.test_worker_fetch(location.origin + "/stream");
        mark("eight-worker stream passed");
        check(responses === 8, "All eight worker requests must use the production Fetch pump");
        check(streams.every(record => record.pulled === 32768 && record.closed), "Every stream must reach EOF");

        globalThis[stateHook] = url => waitFor(() => {
            const record = records.get(url);
            if (!record) return false;
            if (record.case === "drop-request") return typeof record.release === "function";
            if (record.case === "drop-read") return record.pulls === 1;
            // Eight buffered chunks and one pending send bound the producer.
            check(record.pulls <= 9, "Response producer exceeded bounded backpressure");
            return record.pulls === 9;
        }, `production stream state for ${url}`);

        globalThis.fetch = request => {
            check(request instanceof Request, "The main thread must construct the Fetch Request");
            const url = new URL(request.url);
            const testCase = url.pathname.split("/").at(-1);
            check(!records.has(request.url), `Unexpected duplicate request: ${request.url}`);
            const record = { case: testCase, pulls: 0, cancels: 0 };
            records.set(request.url, record);
            if (testCase.startsWith("options-")) {
                const expected = {
                    "options-default": ["same-origin", "follow"],
                    "options-omit": ["omit", "manual"],
                    "options-include": ["include", "follow"],
                }[testCase];
                check(expected, `Unexpected options case: ${testCase}`);
                check(request.credentials === expected[0], `Wrong credentials: ${request.credentials}`);
                check(request.redirect === expected[1], `Wrong redirect policy: ${request.redirect}`);
                check(request.method === "POST", "Request method must be preserved");
                check(request.headers.get("x-fetch-test") === "request", "Request headers must be preserved");
                check(request.headers.get("content-type") === "application/octet-stream", "Content type must be preserved");
                return request.text().then(body => {
                    check(body === "request body", "The asynchronous request body must be buffered correctly");
                    return new Response("response body", { status: 201, headers: { "x-fetch-test": "response" } });
                });
            }
            switch (testCase) {
                case "empty": return Promise.resolve(new Response(null, { status: 204 }));
                case "read-error": {
                    let pulled = false;
                    return Promise.resolve(new Response(new ReadableStream({
                        pull(controller) {
                            if (pulled) controller.error(new Error("fixture stream failure"));
                            else { pulled = true; controller.enqueue(new TextEncoder().encode("prefix")); }
                        },
                    }, { highWaterMark: 0 })));
                }
                case "non-byte": return Promise.resolve(new Response(new ReadableStream({
                    pull(controller) { controller.enqueue("not bytes"); },
                }, { highWaterMark: 0 })));
                case "rejected": return Promise.reject(new Error("fixture Fetch rejection"));
                case "thrown": throw new Error("fixture Fetch throw");
                case "wrong-response": return Promise.resolve({});
                case "drop-read":
                case "drop-backpressure": return Promise.resolve(new Response(makeDropStream(record)));
                case "drop-request": return new Promise(resolve => {
                    record.release = () => resolve(new Response(makeDropStream(record)));
                });
                default: throw new Error(`Unexpected Fetch request: ${request.url}`);
            }
        };

        const modes = [["workers", true], ["single-threaded", false]];
        for (const [mode, allowWorkers] of modes) {
            const base = `${location.origin}/controls/${mode}`;
            mark(`${mode} controls`);
            await fixture.test_fetch_controls(base, allowWorkers);
            // The request future is gone before this delayed Fetch response arrives.
            const pending = records.get(`${base}/drop-request`);
            check(pending && typeof pending.release === "function", "Fetch must have started before request cancellation");
            pending.release();
            for (const testCase of ["drop-read", "drop-backpressure", "drop-request"]) {
                const record = records.get(`${base}/${testCase}`);
                check(record, `Missing drop control: ${testCase}`);
                await waitFor(() => record.cancels === 1, `${testCase} cancellation`);
                check(record.pulls <= (testCase === "drop-backpressure" ? 9 : 1), "Body drop must bound read-ahead");
            }
            mark(`${mode} controls passed`);
            check(!records.has(`${base}/invalid-header`), "Invalid headers must fail before Fetch");
        }
        await new Promise(resolve => setTimeout(resolve, 20));
        const dropRecords = [...records.values()].filter(record => record.case.startsWith("drop-"));
        check(dropRecords.length === 6, "Both consumer modes must execute every drop control");
        check(dropRecords.every(record => record.cancels === 1 && record.pulls === record.pullsAtCancel),
            "Canceled streams must stop producing and be canceled exactly once");
        return {
            chunksPerResponse: 32768,
            workerResponses: responses,
            missingWaitAsyncFallback: true,
            controlModes: modes.map(([mode]) => mode),
            bodyDropCases: dropRecords.length,
        };
    } finally {
        globalThis.fetch = originalFetch;
        delete globalThis[stateHook];
    }
}
