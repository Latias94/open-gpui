// From this directory, build the single MAIN Trunk pipeline and serve its output
// with the shared-memory isolation headers from trunk.toml:
// trunk build queue-test.html --filehash false --locked --html-output index.html
// node test_queue.mjs http://127.0.0.1:8080 [worker-count]
// Defaults to the complete 8- and 16-worker matrix, each with 524,288 items.
// Requires Node's built-in fetch/WebSocket. Set CHROME if detection fails.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { execFile, spawn } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

assert.equal(typeof WebSocket, "function", "Node with a global WebSocket is required");
const origin = new URL(process.argv[2] ?? "http://127.0.0.1:8080").origin;
const workerCounts = process.argv[3] === undefined ? [8, 16] : [Number(process.argv[3])];
assert.ok(workerCounts.every(count => Number.isInteger(count) && count > 0), "Worker count must be a positive integer");
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
const outputRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "target", "queue-test");
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
    console.error(`Browser queue test exceeded its external deadline; see ${output}/chrome.log`);
    void shutdownBrowser().catch(error => console.error("Browser cleanup failed:", error)).finally(() => {
        socket?.close();
        process.exitCode = 1;
    });
}, 300_000);
watchdog.unref();

try {
    let port;
    for (let attempt = 0; attempt < 100; attempt++) {
        if (launchError) throw launchError;
        if (exited()) throw new Error(`Browser exited; see ${output}/chrome.log`);
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
        // This deadline runs outside the page, so a blocked main thread cannot hide a hang.
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
    for (const workers of workerCounts) {
        const result = await call("Runtime.evaluate", {
            awaitPromise: true, returnByValue: true,
            expression: `(${runBrowserTest.toString()})(${workers})`,
        });
        assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails));
        assert.deepEqual(errors, [], JSON.stringify(errors));
        assert.deepEqual(result.result.value, {
            workers,
            items: 524288,
            rounds: 128,
            forbiddenMainThreadWait: true,
            joinedWorkers: workers,
        });
        console.log("PASS: main-thread/worker queue contention", result.result.value);
    }
} finally {
    clearTimeout(watchdog);
    socket?.close();
    try { await shutdownBrowser(); }
    finally { fs.closeSync(log); }
}

async function runBrowserTest(workers) {
    const check = (condition, message) => { if (!condition) throw new Error(message); };
    check(crossOriginIsolated, "Shared-memory isolation headers are required");
    const fixture = await import("/queue-test.js");
    const bindings = await fixture.default();
    check(bindings.memory.buffer instanceof SharedArrayBuffer, "Wasm memory must be shared");
    let trapped = false;
    try { fixture.forbidden_main_thread_wait(); }
    catch (error) { trapped = error instanceof WebAssembly.RuntimeError; }
    check(trapped, "Main-thread Wasm waits must trap in this browser");

    const rounds = 128, batch = 4096;
    const stress = new fixture.QueueStress(workers, rounds * batch);
    const yieldToBrowser = () => new Promise(resolve => setTimeout(resolve, 2));
    console.info(`OPEN_GPUI_QUEUE_STAGE ${workers} workers starting`);
    while (!stress.ready()) await yieldToBrowser();
    for (let round = 0; round < rounds; round++) {
        stress.send_batch(batch);
        while (stress.drain() !== (round + 1) * batch) await yieldToBrowser();
        // Empty the request queue so workers repeatedly park between busy rounds.
        await yieldToBrowser();
    }
    stress.stop();
    while (!stress.stopped()) await yieldToBrowser();
    const joinedWorkers = await stress.join_workers();
    check(joinedWorkers === workers, "Every worker must be joined before the fixture is freed");
    stress.free();
    console.info(`OPEN_GPUI_QUEUE_STAGE ${workers} workers joined`);
    return { workers, items: rounds * batch, rounds, forbiddenMainThreadWait: trapped, joinedWorkers };
}
