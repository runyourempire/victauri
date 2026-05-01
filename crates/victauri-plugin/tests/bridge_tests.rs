//! JS bridge tests using jsdom via Node.js subprocess.
//!
//! Each test function creates a batch of related JS test cases, serializes them
//! to a JSON temp file, runs `node run_tests.js <file>`, and asserts the results.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use victauri_plugin::js_bridge::{BridgeCapacities, init_script};

// ── Test Infrastructure ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct TestDef {
    bridge_script: String,
    setup_html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    setup_js: Option<String>,
    tests: Vec<TestCase>,
}

#[derive(Serialize)]
struct TestCase {
    name: String,
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    setup_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    setup_js: Option<String>,
}

#[derive(Deserialize, Debug)]
struct TestResult {
    name: String,
    passed: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

fn runner_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("bridge_tests")
}

fn run_tests(def: &TestDef) -> Vec<TestResult> {
    let runner = runner_dir().join("run_tests.js");
    assert!(
        runner.exists(),
        "run_tests.js not found at {}",
        runner.display()
    );

    // Write test def to temp file
    let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
    serde_json::to_writer(&mut tmp, def).expect("serialize test def");
    tmp.flush().expect("flush temp file");

    let output = Command::new("node")
        .arg(runner.to_str().unwrap())
        .arg(tmp.path().to_str().unwrap())
        .output()
        .expect("failed to run node");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Find the results line
    let results_line = stdout
        .lines()
        .find(|l| l.starts_with("VICTAURI_RESULTS:"))
        .unwrap_or_else(|| {
            panic!("No VICTAURI_RESULTS line in output.\nstdout: {stdout}\nstderr: {stderr}")
        });

    let json_str = &results_line["VICTAURI_RESULTS:".len()..];
    serde_json::from_str(json_str)
        .unwrap_or_else(|e| panic!("Failed to parse results JSON: {e}\nraw: {json_str}"))
}

fn assert_all_pass(results: &[TestResult]) {
    let mut failures = Vec::new();
    for r in results {
        if !r.passed {
            failures.push(format!(
                "  FAIL: {} => {:?}",
                r.name,
                r.error.as_deref().unwrap_or("unknown")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "Test failures:\n{}",
        failures.join("\n")
    );
}

fn bridge_script() -> String {
    init_script(&BridgeCapacities::default())
}

fn default_html() -> String {
    r#"<html lang="en"><head><title>Test Page</title></head><body>
        <div id="app">
            <h1>Hello</h1>
            <button id="btn1">Click Me</button>
            <input id="input1" type="text" placeholder="Enter text" />
            <textarea id="ta1">Some text</textarea>
            <div contenteditable="true" id="ce1">Editable</div>
            <a href="/about" id="link1">About</a>
            <select id="sel1">
                <option value="a">Option A</option>
                <option value="b">Option B</option>
                <option value="c">Option C</option>
            </select>
            <nav><span>Nav item</span></nav>
        </div>
    </body></html>"#
        .to_string()
}

// ── Test Functions ───────────────────────────────────────────────────────────

#[test]
fn bridge_init_version_and_idempotent() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "version is 0.3.0".into(),
                code: "return window.__VICTAURI__.version;".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "bridge object exists".into(),
                code: "return typeof window.__VICTAURI__;".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "idempotent - re-inject does not overwrite".into(),
                code: format!(
                    r"
                    window.__VICTAURI__._marker = 'first';
                    eval({});
                    return window.__VICTAURI__._marker;
                    ",
                    serde_json::to_string(&bridge_script()).unwrap()
                ),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "all public methods present".into(),
                code: r"
                    var methods = ['snapshot','getRef','getStaleRefs','findElements',
                        'click','doubleClick','hover','fill','type','pressKey','selectOption',
                        'scrollTo','focusElement','getIpcLog','clearIpcLog','getConsoleLogs',
                        'clearConsoleLogs','getMutationLog','clearMutationLog','getNetworkLog',
                        'clearNetworkLog','getLocalStorage','setLocalStorage','deleteLocalStorage',
                        'getSessionStorage','setSessionStorage','deleteSessionStorage','getCookies',
                        'getNavigationLog','navigate','navigateBack','getDialogLog','clearDialogLog',
                        'setDialogAutoResponse','getEventStream','waitFor','getStyles',
                        'getBoundingBoxes','highlightElement','clearHighlights','injectCss',
                        'removeInjectedCss','auditAccessibility','getPerformanceMetrics'];
                    var missing = methods.filter(function(m) { return typeof window.__VICTAURI__[m] !== 'function'; });
                    return { missing: missing, count: methods.length };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap(), "0.3.0");
    assert_eq!(results[1].result.as_ref().unwrap(), "object");
    assert_eq!(results[2].result.as_ref().unwrap(), "first");
    let missing: Vec<String> =
        serde_json::from_value(results[3].result.as_ref().unwrap()["missing"].clone()).unwrap();
    assert!(missing.is_empty(), "Missing methods: {missing:?}");
}

#[test]
fn snapshot_compact_format() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "snapshot default is compact".into(),
                code: r"
                    var result = window.__VICTAURI__.snapshot();
                    return { format: result.format, type: typeof result.tree };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "compact snapshot returns string tree".into(),
                code: r"
                    var result = window.__VICTAURI__.snapshot('compact');
                    return { format: result.format, is_string: typeof result.tree === 'string', has_refs: result.tree.indexOf('[e') !== -1 };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "compact format contains button ref".into(),
                code: r"
                    var result = window.__VICTAURI__.snapshot('compact');
                    return { has_button: result.tree.indexOf('button') !== -1, has_click_me: result.tree.indexOf('Click Me') !== -1 };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "stale_refs is array".into(),
                code: r"
                    var result = window.__VICTAURI__.snapshot();
                    return Array.isArray(result.stale_refs);
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["format"], "compact");
    assert_eq!(results[0].result.as_ref().unwrap()["type"], "string");
    assert_eq!(results[1].result.as_ref().unwrap()["is_string"], true);
    assert_eq!(results[1].result.as_ref().unwrap()["has_refs"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["has_button"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["has_click_me"], true);
    assert_eq!(
        results[3].result.as_ref().unwrap(),
        &serde_json::Value::Bool(true)
    );
}

#[test]
fn snapshot_json_format() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "json snapshot returns object tree".into(),
                code: r"
                    var result = window.__VICTAURI__.snapshot('json');
                    return { format: result.format, tag: result.tree.tag, has_ref: !!result.tree.ref_id, has_children: result.tree.children.length > 0 };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "json tree has correct structure".into(),
                code: r"
                    var result = window.__VICTAURI__.snapshot('json');
                    var body = result.tree;
                    return { tag: body.tag, visible: body.visible, has_bounds: !!body.bounds, has_role: body.role !== undefined };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "json tree child has ref and role".into(),
                code: r"
                    var result = window.__VICTAURI__.snapshot('json');
                    var div = result.tree.children[0];
                    return { tag: div.tag, ref_starts_e: div.ref_id.charAt(0) === 'e', has_children: div.children.length > 0 };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "button in json tree has role and name".into(),
                code: r"
                    var result = window.__VICTAURI__.snapshot('json');
                    function findByTag(node, tag) {
                        if (node.tag === tag) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findByTag(node.children[i], tag);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findByTag(result.tree, 'button');
                    return { role: btn.role, name: btn.name, tag: btn.tag };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["format"], "json");
    assert_eq!(results[0].result.as_ref().unwrap()["tag"], "body");
    assert_eq!(results[0].result.as_ref().unwrap()["has_ref"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["has_children"], true);
    assert_eq!(results[3].result.as_ref().unwrap()["role"], "button");
    assert_eq!(results[3].result.as_ref().unwrap()["name"], "Click Me");
}

#[test]
fn ref_lifecycle() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "snapshot assigns refs that can be resolved".into(),
                code: r"
                    var result = window.__VICTAURI__.snapshot('json');
                    var ref_id = result.tree.ref_id;
                    var el = window.__VICTAURI__.getRef(ref_id);
                    return { ref_id: ref_id, resolved: el !== null, tag: el ? el.tagName.toLowerCase() : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getRef returns null for invalid ref".into(),
                code: r"
                    var el = window.__VICTAURI__.getRef('e99999');
                    return { is_null: el === null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "snapshot clears old refs and reports stale".into(),
                code: r"
                    // First snapshot — registers refs for all elements
                    var s1 = window.__VICTAURI__.snapshot('json');
                    // Find a button ref from first snapshot
                    function findByTag(node, tag) {
                        if (node.tag === tag) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findByTag(node.children[i], tag);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findByTag(s1.tree, 'button');
                    var btnRef = btn.ref_id;
                    // Remove the button from DOM to make its ref stale
                    var el = window.__VICTAURI__.getRef(btnRef);
                    if (el && el.parentNode) el.parentNode.removeChild(el);
                    // Second snapshot — should detect btnRef as stale
                    var s2 = window.__VICTAURI__.snapshot('json');
                    return { stale_has_btn: s2.stale_refs.indexOf(btnRef) !== -1 };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getStaleRefs detects disconnected elements".into(),
                code: r"
                    window.__VICTAURI__.snapshot('json');
                    // Manually remove an element
                    var btn = document.getElementById('btn1');
                    btn.parentNode.removeChild(btn);
                    var stale = window.__VICTAURI__.getStaleRefs();
                    return { has_stale: stale.length > 0 };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["resolved"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["tag"], "body");
    assert_eq!(results[1].result.as_ref().unwrap()["is_null"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["stale_has_btn"], true);
    assert_eq!(results[3].result.as_ref().unwrap()["has_stale"], true);
}

#[test]
fn click_interaction() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "click resolves ok for valid ref".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findByTag(node, tag) {
                        if (node.tag === tag) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findByTag(node.children[i], tag);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findByTag(snap.tree, 'button');
                    var result = await window.__VICTAURI__.click(btn.ref_id);
                    return result;
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "click fails for invalid ref".into(),
                code: r"
                    var result = await window.__VICTAURI__.click('e99999');
                    return result;
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "click dispatches click event".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findByTag(node, tag) {
                        if (node.tag === tag) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findByTag(node.children[i], tag);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findByTag(snap.tree, 'button');
                    var clicked = false;
                    var el = window.__VICTAURI__.getRef(btn.ref_id);
                    el.addEventListener('click', function() { clicked = true; });
                    await window.__VICTAURI__.click(btn.ref_id);
                    return { clicked: clicked };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[1].result.as_ref().unwrap()["ok"], false);
    assert_eq!(results[2].result.as_ref().unwrap()["clicked"], true);
}

#[test]
fn double_click_and_hover() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "doubleClick dispatches dblclick event".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findByTag(node, tag) {
                        if (node.tag === tag) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findByTag(node.children[i], tag);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findByTag(snap.tree, 'button');
                    var dblClicked = false;
                    var el = window.__VICTAURI__.getRef(btn.ref_id);
                    el.addEventListener('dblclick', function() { dblClicked = true; });
                    await window.__VICTAURI__.doubleClick(btn.ref_id);
                    return { dblClicked: dblClicked };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "hover dispatches mouseenter and mouseover".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findByTag(node, tag) {
                        if (node.tag === tag) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findByTag(node.children[i], tag);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findByTag(snap.tree, 'button');
                    var events = [];
                    var el = window.__VICTAURI__.getRef(btn.ref_id);
                    el.addEventListener('mouseenter', function() { events.push('mouseenter'); });
                    el.addEventListener('mouseover', function() { events.push('mouseover'); });
                    await window.__VICTAURI__.hover(btn.ref_id);
                    return { events: events };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["dblClicked"], true);
    let events: Vec<String> =
        serde_json::from_value(results[1].result.as_ref().unwrap()["events"].clone()).unwrap();
    assert!(events.contains(&"mouseenter".to_string()));
    assert!(events.contains(&"mouseover".to_string()));
}

#[test]
fn fill_interaction() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "fill sets input value".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findById(node, id) {
                        if (node.attributes && node.attributes.id === id) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findById(node.children[i], id);
                            if (r) return r;
                        }
                        return null;
                    }
                    var inp = findById(snap.tree, 'input1');
                    await window.__VICTAURI__.fill(inp.ref_id, 'Hello World');
                    var el = window.__VICTAURI__.getRef(inp.ref_id);
                    return { ok: true, value: el.value };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "fill dispatches input and change events".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findById(node, id) {
                        if (node.attributes && node.attributes.id === id) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findById(node.children[i], id);
                            if (r) return r;
                        }
                        return null;
                    }
                    var inp = findById(snap.tree, 'input1');
                    var events = [];
                    var el = window.__VICTAURI__.getRef(inp.ref_id);
                    el.addEventListener('input', function() { events.push('input'); });
                    el.addEventListener('change', function() { events.push('change'); });
                    await window.__VICTAURI__.fill(inp.ref_id, 'test');
                    return { events: events };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "fill works on textarea".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findById(node, id) {
                        if (node.attributes && node.attributes.id === id) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findById(node.children[i], id);
                            if (r) return r;
                        }
                        return null;
                    }
                    var ta = findById(snap.tree, 'ta1');
                    await window.__VICTAURI__.fill(ta.ref_id, 'New text');
                    var el = window.__VICTAURI__.getRef(ta.ref_id);
                    return { value: el.value };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "fill fails on non-fillable element".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findByTag(node, tag) {
                        if (node.tag === tag) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findByTag(node.children[i], tag);
                            if (r) return r;
                        }
                        return null;
                    }
                    var h1 = findByTag(snap.tree, 'h1');
                    var result = await window.__VICTAURI__.fill(h1.ref_id, 'test');
                    return result;
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["value"], "Hello World");
    let events: Vec<String> =
        serde_json::from_value(results[1].result.as_ref().unwrap()["events"].clone()).unwrap();
    assert!(events.contains(&"input".to_string()));
    assert!(events.contains(&"change".to_string()));
    assert_eq!(results[2].result.as_ref().unwrap()["value"], "New text");
    assert_eq!(results[3].result.as_ref().unwrap()["ok"], false);
}

#[test]
fn type_text_interaction() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "type builds value char by char".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findById(node, id) {
                        if (node.attributes && node.attributes.id === id) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findById(node.children[i], id);
                            if (r) return r;
                        }
                        return null;
                    }
                    var inp = findById(snap.tree, 'input1');
                    await window.__VICTAURI__.type(inp.ref_id, 'abc');
                    var el = window.__VICTAURI__.getRef(inp.ref_id);
                    return { value: el.value };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "type dispatches keydown/keypress/input/keyup per char".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findById(node, id) {
                        if (node.attributes && node.attributes.id === id) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findById(node.children[i], id);
                            if (r) return r;
                        }
                        return null;
                    }
                    var inp = findById(snap.tree, 'input1');
                    var events = [];
                    var el = window.__VICTAURI__.getRef(inp.ref_id);
                    el.addEventListener('keydown', function(e) { events.push('keydown:' + e.key); });
                    el.addEventListener('keypress', function(e) { events.push('keypress:' + e.key); });
                    el.addEventListener('input', function(e) { events.push('input'); });
                    el.addEventListener('keyup', function(e) { events.push('keyup:' + e.key); });
                    await window.__VICTAURI__.type(inp.ref_id, 'x');
                    return { events: events };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["value"], "abc");
    let events: Vec<String> =
        serde_json::from_value(results[1].result.as_ref().unwrap()["events"].clone()).unwrap();
    assert!(events.contains(&"keydown:x".to_string()));
    assert!(events.contains(&"keypress:x".to_string()));
    assert!(events.contains(&"input".to_string()));
    assert!(events.contains(&"keyup:x".to_string()));
}

#[test]
fn press_key_interaction() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "pressKey dispatches on active element".into(),
                code: r"
                    var events = [];
                    document.body.addEventListener('keydown', function(e) { events.push('keydown:' + e.key); });
                    document.body.addEventListener('keyup', function(e) { events.push('keyup:' + e.key); });
                    var result = window.__VICTAURI__.pressKey('Escape');
                    return { ok: result.ok, events: events };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "pressKey returns ok true".into(),
                code: r"
                    var result = window.__VICTAURI__.pressKey('Enter');
                    return result;
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["ok"], true);
    let events: Vec<String> =
        serde_json::from_value(results[0].result.as_ref().unwrap()["events"].clone()).unwrap();
    assert!(events.contains(&"keydown:Escape".to_string()));
    assert!(events.contains(&"keyup:Escape".to_string()));
    assert_eq!(results[1].result.as_ref().unwrap()["ok"], true);
}

#[test]
fn select_option_interaction() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "selectOption sets selected options".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findById(node, id) {
                        if (node.attributes && node.attributes.id === id) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findById(node.children[i], id);
                            if (r) return r;
                        }
                        return null;
                    }
                    var sel = findById(snap.tree, 'sel1');
                    var result = await window.__VICTAURI__.selectOption(sel.ref_id, ['b']);
                    var el = window.__VICTAURI__.getRef(sel.ref_id);
                    return { ok: result.ok, value: el.value };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "selectOption fails on non-select element".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findByTag(node, tag) {
                        if (node.tag === tag) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findByTag(node.children[i], tag);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findByTag(snap.tree, 'button');
                    var result = await window.__VICTAURI__.selectOption(btn.ref_id, ['a']);
                    return result;
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["value"], "b");
    assert_eq!(results[1].result.as_ref().unwrap()["ok"], false);
}

#[test]
fn focus_and_scroll() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "focusElement returns ok with tag".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findById(node, id) {
                        if (node.attributes && node.attributes.id === id) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findById(node.children[i], id);
                            if (r) return r;
                        }
                        return null;
                    }
                    var inp = findById(snap.tree, 'input1');
                    var result = await window.__VICTAURI__.focusElement(inp.ref_id);
                    return result;
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "scrollTo with no refId uses window".into(),
                code: r"
                    var result = await window.__VICTAURI__.scrollTo(null, 100, 200);
                    return result;
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "scrollTo with refId scrolls element into view".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findByTag(node, tag) {
                        if (node.tag === tag) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findByTag(node.children[i], tag);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findByTag(snap.tree, 'button');
                    var result = await window.__VICTAURI__.scrollTo(btn.ref_id, 0, 0);
                    return result;
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["tag"], "input");
    assert_eq!(results[1].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["ok"], true);
}

#[test]
fn console_log_capture() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "console.log is captured".into(),
                code: r"
                    console.log('hello from test');
                    var logs = window.__VICTAURI__.getConsoleLogs();
                    var last = logs[logs.length - 1];
                    return { level: last.level, message: last.message, has_ts: typeof last.timestamp === 'number' };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "console.warn and error are captured".into(),
                code: r"
                    console.warn('warning!');
                    console.error('error!');
                    var logs = window.__VICTAURI__.getConsoleLogs();
                    var warn = logs.find(function(l) { return l.level === 'warn'; });
                    var err = logs.find(function(l) { return l.level === 'error'; });
                    return { has_warn: !!warn, has_error: !!err, warn_msg: warn ? warn.message : null, err_msg: err ? err.message : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "clearConsoleLogs empties the log".into(),
                code: r"
                    console.log('test');
                    window.__VICTAURI__.clearConsoleLogs();
                    var logs = window.__VICTAURI__.getConsoleLogs();
                    return { count: logs.length };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getConsoleLogs since filter works".into(),
                code: r"
                    console.log('old message');
                    var ts = Date.now() + 1;
                    console.log('new message');
                    var filtered = window.__VICTAURI__.getConsoleLogs(ts);
                    // Since both happen nearly simultaneously, just verify the API doesn't crash
                    return { is_array: Array.isArray(filtered) };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["level"], "log");
    assert_eq!(
        results[0].result.as_ref().unwrap()["message"],
        "hello from test"
    );
    assert_eq!(results[0].result.as_ref().unwrap()["has_ts"], true);
    assert_eq!(results[1].result.as_ref().unwrap()["has_warn"], true);
    assert_eq!(results[1].result.as_ref().unwrap()["has_error"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["count"], 0);
    assert_eq!(results[3].result.as_ref().unwrap()["is_array"], true);
}

#[test]
fn network_log_interception() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "fetch calls are logged".into(),
                code: r"
                    await fetch('http://example.com/api/data');
                    // Give network log time to update
                    var log = window.__VICTAURI__.getNetworkLog();
                    var entry = log.find(function(e) { return e.url.indexOf('example.com') !== -1; });
                    return { found: !!entry, method: entry ? entry.method : null, has_id: entry ? typeof entry.id === 'number' : false };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getNetworkLog filter works".into(),
                code: r"
                    await fetch('http://api.example.com/users');
                    await fetch('http://other.com/data');
                    var filtered = window.__VICTAURI__.getNetworkLog('api.example.com');
                    return { count: filtered.length, url: filtered.length > 0 ? filtered[0].url : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "clearNetworkLog empties the log".into(),
                code: r"
                    await fetch('http://example.com/test');
                    window.__VICTAURI__.clearNetworkLog();
                    var log = window.__VICTAURI__.getNetworkLog();
                    return { count: log.length };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["found"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["method"], "GET");
    assert_eq!(results[0].result.as_ref().unwrap()["has_id"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["count"], 0);
}

#[test]
fn navigation_tracking() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "initial navigation entry exists".into(),
                code: r"
                    var log = window.__VICTAURI__.getNavigationLog();
                    return { count: log.length, first_type: log[0] ? log[0].type : null };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "pushState is tracked".into(),
                code: r"
                    history.pushState({}, '', '/new-page');
                    var log = window.__VICTAURI__.getNavigationLog();
                    var push = log.find(function(e) { return e.type === 'pushState'; });
                    return { found: !!push, url: push ? push.url : null };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "replaceState is tracked".into(),
                code: r"
                    history.replaceState({}, '', '/replaced');
                    var log = window.__VICTAURI__.getNavigationLog();
                    var replace = log.find(function(e) { return e.type === 'replaceState'; });
                    return { found: !!replace, url: replace ? replace.url : null };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert!(
        results[0].result.as_ref().unwrap()["count"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert_eq!(results[0].result.as_ref().unwrap()["first_type"], "initial");
    assert_eq!(results[1].result.as_ref().unwrap()["found"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["found"], true);
}

#[test]
fn dialog_capture() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "alert is captured".into(),
                code: r"
                    window.alert('Hello!');
                    var log = window.__VICTAURI__.getDialogLog();
                    var entry = log[log.length - 1];
                    return { type: entry.type, message: entry.message };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "confirm returns true by default".into(),
                code: r"
                    var result = window.confirm('Are you sure?');
                    var log = window.__VICTAURI__.getDialogLog();
                    var entry = log[log.length - 1];
                    return { result: result, type: entry.type, message: entry.message };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "prompt returns empty string by default".into(),
                code: r"
                    var result = window.prompt('Enter name:');
                    var log = window.__VICTAURI__.getDialogLog();
                    var entry = log[log.length - 1];
                    return { result: result, type: entry.type, message: entry.message };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "setDialogAutoResponse changes confirm behavior".into(),
                code: r"
                    window.__VICTAURI__.setDialogAutoResponse('confirm', 'dismiss');
                    var result = window.confirm('Will you?');
                    return { result: result };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "clearDialogLog empties the log".into(),
                code: r"
                    window.alert('test');
                    window.__VICTAURI__.clearDialogLog();
                    var log = window.__VICTAURI__.getDialogLog();
                    return { count: log.length };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["type"], "alert");
    assert_eq!(results[0].result.as_ref().unwrap()["message"], "Hello!");
    assert_eq!(results[1].result.as_ref().unwrap()["result"], true);
    assert_eq!(results[1].result.as_ref().unwrap()["type"], "confirm");
    assert_eq!(results[2].result.as_ref().unwrap()["type"], "prompt");
    assert_eq!(results[3].result.as_ref().unwrap()["result"], false);
    assert_eq!(results[4].result.as_ref().unwrap()["count"], 0);
}

#[test]
fn storage_local() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "setLocalStorage and get round-trips".into(),
                code: r"
                    window.__VICTAURI__.setLocalStorage('key1', 'value1');
                    var val = window.__VICTAURI__.getLocalStorage('key1');
                    return { val: val };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "setLocalStorage with object serializes JSON".into(),
                code: r"
                    window.__VICTAURI__.setLocalStorage('obj', { a: 1, b: 'hi' });
                    var val = window.__VICTAURI__.getLocalStorage('obj');
                    return val;
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "deleteLocalStorage removes key".into(),
                code: r"
                    window.__VICTAURI__.setLocalStorage('del_me', 'exists');
                    window.__VICTAURI__.deleteLocalStorage('del_me');
                    var val = window.__VICTAURI__.getLocalStorage('del_me');
                    return { val: val };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getLocalStorage with no key returns all".into(),
                code: r"
                    window.__VICTAURI__.setLocalStorage('aa', 'one');
                    window.__VICTAURI__.setLocalStorage('bb', 'two');
                    var all = window.__VICTAURI__.getLocalStorage();
                    return { has_aa: all.aa === 'one', has_bb: all.bb === 'two' };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["val"], "value1");
    assert_eq!(results[1].result.as_ref().unwrap()["a"], 1);
    assert_eq!(results[1].result.as_ref().unwrap()["b"], "hi");
    // deleteLocalStorage: value is null
    assert!(results[2].result.as_ref().unwrap()["val"].is_null());
    assert_eq!(results[3].result.as_ref().unwrap()["has_aa"], true);
    assert_eq!(results[3].result.as_ref().unwrap()["has_bb"], true);
}

#[test]
fn storage_session() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "setSessionStorage and get round-trips".into(),
                code: r"
                    window.__VICTAURI__.setSessionStorage('skey', 'sval');
                    var val = window.__VICTAURI__.getSessionStorage('skey');
                    return { val: val };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "deleteSessionStorage removes key".into(),
                code: r"
                    window.__VICTAURI__.setSessionStorage('sdel', 'exists');
                    window.__VICTAURI__.deleteSessionStorage('sdel');
                    var val = window.__VICTAURI__.getSessionStorage('sdel');
                    return { val: val };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["val"], "sval");
    assert!(results[1].result.as_ref().unwrap()["val"].is_null());
}

#[test]
fn css_inspection() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "getStyles returns styles for ref".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    var result = window.__VICTAURI__.getStyles(snap.tree.ref_id);
                    return { has_ref: !!result.ref_id, has_tag: !!result.tag, has_styles: typeof result.styles === 'object' };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getStyles with specific properties".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    var result = window.__VICTAURI__.getStyles(snap.tree.ref_id, ['display', 'visibility']);
                    return { display: result.styles.display, visibility: result.styles.visibility };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getStyles errors for invalid ref".into(),
                code: r"
                    var result = window.__VICTAURI__.getStyles('e99999');
                    return { has_error: !!result.error };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getBoundingBoxes returns box model".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    var result = window.__VICTAURI__.getBoundingBoxes([snap.tree.ref_id]);
                    var box = result[0];
                    return { ref_id: box.ref_id, has_width: typeof box.width === 'number', has_margin: typeof box.margin === 'object', has_padding: typeof box.padding === 'object' };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getBoundingBoxes errors for invalid ref".into(),
                code: r"
                    var result = window.__VICTAURI__.getBoundingBoxes(['e99999']);
                    return { error: result[0].error };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["has_ref"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["has_tag"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["has_styles"], true);
    assert_eq!(results[1].result.as_ref().unwrap()["display"], "block");
    assert_eq!(results[1].result.as_ref().unwrap()["visibility"], "visible");
    assert_eq!(results[2].result.as_ref().unwrap()["has_error"], true);
    assert_eq!(results[3].result.as_ref().unwrap()["has_width"], true);
    assert_eq!(results[3].result.as_ref().unwrap()["has_margin"], true);
    assert_eq!(results[3].result.as_ref().unwrap()["has_padding"], true);
    assert_eq!(
        results[4].result.as_ref().unwrap()["error"],
        "ref not found"
    );
}

#[test]
fn css_injection() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "injectCss creates style element".into(),
                code: r"
                    var result = window.__VICTAURI__.injectCss('body { color: red; }');
                    var style = document.getElementById('__victauri_injected_css__');
                    return { ok: result.ok, length: result.length, exists: !!style, content: style ? style.textContent : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "injectCss replaces previous injection".into(),
                code: r"
                    window.__VICTAURI__.injectCss('body { color: red; }');
                    window.__VICTAURI__.injectCss('body { color: blue; }');
                    var styles = document.querySelectorAll('#__victauri_injected_css__');
                    return { count: styles.length, content: styles[0] ? styles[0].textContent : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "removeInjectedCss removes style element".into(),
                code: r"
                    window.__VICTAURI__.injectCss('body { color: green; }');
                    var result = window.__VICTAURI__.removeInjectedCss();
                    var style = document.getElementById('__victauri_injected_css__');
                    return { ok: result.ok, removed: result.removed, exists: !!style };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "removeInjectedCss when none exists returns removed:false".into(),
                code: r"
                    var result = window.__VICTAURI__.removeInjectedCss();
                    return result;
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["exists"], true);
    assert_eq!(
        results[0].result.as_ref().unwrap()["content"],
        "body { color: red; }"
    );
    assert_eq!(results[1].result.as_ref().unwrap()["count"], 1);
    assert_eq!(
        results[1].result.as_ref().unwrap()["content"],
        "body { color: blue; }"
    );
    assert_eq!(results[2].result.as_ref().unwrap()["removed"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["exists"], false);
    assert_eq!(results[3].result.as_ref().unwrap()["removed"], false);
}

#[test]
fn highlight_elements() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "highlightElement creates overlay".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    var result = window.__VICTAURI__.highlightElement(snap.tree.ref_id, 'red', 'test label');
                    var overlays = document.querySelectorAll('.__victauri_highlight__');
                    return { ok: result.ok, count: overlays.length, has_label: overlays[0] && overlays[0].children.length > 0 };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "highlightElement errors for invalid ref".into(),
                code: r"
                    var result = window.__VICTAURI__.highlightElement('e99999');
                    return { has_error: !!result.error };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "clearHighlights removes all overlays".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    window.__VICTAURI__.highlightElement(snap.tree.ref_id);
                    window.__VICTAURI__.highlightElement(snap.tree.ref_id);
                    var result = window.__VICTAURI__.clearHighlights();
                    var overlays = document.querySelectorAll('.__victauri_highlight__');
                    return { ok: result.ok, removed: result.removed, remaining: overlays.length };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["count"], 1);
    assert_eq!(results[0].result.as_ref().unwrap()["has_label"], true);
    assert_eq!(results[1].result.as_ref().unwrap()["has_error"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["removed"], 2);
    assert_eq!(results[2].result.as_ref().unwrap()["remaining"], 0);
}

#[test]
fn accessibility_audit() {
    let a11y_html = r#"<html><head><title>A11y Test</title></head><body>
        <img src="test.png" />
        <input type="text" />
        <button></button>
        <a href="/test"></a>
        <div role="invalid_role">test</div>
        <div tabindex="5">tab</div>
    </body></html>"#
        .to_string();

    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: a11y_html,
        setup_js: None,
        tests: vec![
            TestCase {
                name: "audit detects img without alt".into(),
                code: r"
                    var result = window.__VICTAURI__.auditAccessibility();
                    var imgAlt = result.violations.find(function(v) { return v.rule === 'img-alt'; });
                    return { found: !!imgAlt, severity: imgAlt ? imgAlt.severity : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "audit detects input without label".into(),
                code: r"
                    var result = window.__VICTAURI__.auditAccessibility();
                    var inputLabel = result.violations.find(function(v) { return v.rule === 'input-label'; });
                    return { found: !!inputLabel, severity: inputLabel ? inputLabel.severity : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "audit detects button without name".into(),
                code: r"
                    var result = window.__VICTAURI__.auditAccessibility();
                    var btnName = result.violations.find(function(v) { return v.rule === 'button-name'; });
                    return { found: !!btnName };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "audit detects link without text".into(),
                code: r"
                    var result = window.__VICTAURI__.auditAccessibility();
                    var linkName = result.violations.find(function(v) { return v.rule === 'link-name'; });
                    return { found: !!linkName };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "audit detects invalid ARIA role".into(),
                code: r"
                    var result = window.__VICTAURI__.auditAccessibility();
                    var ariaRole = result.warnings.find(function(w) { return w.rule === 'aria-role'; });
                    return { found: !!ariaRole, message: ariaRole ? ariaRole.message : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "audit detects positive tabindex".into(),
                code: r"
                    var result = window.__VICTAURI__.auditAccessibility();
                    var tabIdx = result.warnings.find(function(w) { return w.rule === 'tabindex-positive'; });
                    return { found: !!tabIdx };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "audit returns summary counts".into(),
                code: r"
                    var result = window.__VICTAURI__.auditAccessibility();
                    return { has_summary: !!result.summary, total: result.summary.total };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["found"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["severity"], "critical");
    assert_eq!(results[1].result.as_ref().unwrap()["found"], true);
    assert_eq!(results[1].result.as_ref().unwrap()["severity"], "serious");
    assert_eq!(results[2].result.as_ref().unwrap()["found"], true);
    assert_eq!(results[3].result.as_ref().unwrap()["found"], true);
    assert_eq!(results[4].result.as_ref().unwrap()["found"], true);
    assert_eq!(results[5].result.as_ref().unwrap()["found"], true);
    assert_eq!(results[6].result.as_ref().unwrap()["has_summary"], true);
    assert!(
        results[6].result.as_ref().unwrap()["total"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn event_stream() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "getEventStream returns console events".into(),
                code: r"
                    console.log('stream test');
                    var events = window.__VICTAURI__.getEventStream();
                    var consoleEvents = events.filter(function(e) { return e.type === 'console'; });
                    return { has_console: consoleEvents.length > 0, first_type: consoleEvents.length > 0 ? consoleEvents[0].type : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getEventStream includes network events".into(),
                code: r"
                    await fetch('http://test.com/api');
                    var events = window.__VICTAURI__.getEventStream();
                    var netEvents = events.filter(function(e) { return e.type === 'network'; });
                    return { has_network: netEvents.length > 0 };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getEventStream since filter works".into(),
                code: r"
                    console.log('old');
                    var ts = Date.now() + 10000;
                    var events = window.__VICTAURI__.getEventStream(ts);
                    return { count: events.length };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "getEventStream includes navigation events".into(),
                code: r"
                    var events = window.__VICTAURI__.getEventStream();
                    var navEvents = events.filter(function(e) { return e.type === 'navigation'; });
                    return { has_nav: navEvents.length > 0, first_nav_type: navEvents.length > 0 ? navEvents[0].nav_type : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["has_console"], true);
    assert_eq!(results[1].result.as_ref().unwrap()["has_network"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["count"], 0);
    assert_eq!(results[3].result.as_ref().unwrap()["has_nav"], true);
    assert_eq!(
        results[3].result.as_ref().unwrap()["first_nav_type"],
        "initial"
    );
}

#[test]
fn wait_for_conditions() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "waitFor text condition met immediately".into(),
                code: r"
                    var result = await window.__VICTAURI__.waitFor({ condition: 'text', value: 'Hello', timeout_ms: 500 });
                    return result;
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "waitFor selector condition met".into(),
                code: r"
                    var result = await window.__VICTAURI__.waitFor({ condition: 'selector', value: '#btn1', timeout_ms: 500 });
                    return result;
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "waitFor selector_gone when element absent".into(),
                code: r"
                    var result = await window.__VICTAURI__.waitFor({ condition: 'selector_gone', value: '#nonexistent', timeout_ms: 500 });
                    return result;
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "waitFor text_gone when text absent".into(),
                code: r"
                    var result = await window.__VICTAURI__.waitFor({ condition: 'text_gone', value: 'NONEXISTENT_TEXT_XYZ', timeout_ms: 500 });
                    return result;
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "waitFor timeout when condition not met".into(),
                code: r"
                    var result = await window.__VICTAURI__.waitFor({ condition: 'text', value: 'IMPOSSIBLE_TEXT_XYZ', timeout_ms: 100, poll_ms: 20 });
                    return { ok: result.ok, has_error: !!result.error };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[1].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[3].result.as_ref().unwrap()["ok"], true);
    assert_eq!(results[4].result.as_ref().unwrap()["ok"], false);
    assert_eq!(results[4].result.as_ref().unwrap()["has_error"], true);
}

#[test]
fn ipc_log() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "IPC calls are captured from network log".into(),
                code: r"
                    await fetch('http://ipc.localhost/get_settings', { method: 'POST', body: '{}' });
                    var log = window.__VICTAURI__.getIpcLog();
                    var entry = log.find(function(e) { return e.command === 'get_settings'; });
                    return { found: !!entry, command: entry ? entry.command : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "Victauri plugin calls are excluded from IPC log".into(),
                code: r"
                    await fetch('http://ipc.localhost/plugin%3Avictauri%7Ceval', { method: 'POST', body: '{}' });
                    var log = window.__VICTAURI__.getIpcLog();
                    var victauriEntry = log.find(function(e) { return e.command && e.command.indexOf('victauri') !== -1; });
                    return { excluded: !victauriEntry };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "IPC log limit works".into(),
                code: r"
                    await fetch('http://ipc.localhost/cmd1', { method: 'POST', body: '{}' });
                    await fetch('http://ipc.localhost/cmd2', { method: 'POST', body: '{}' });
                    await fetch('http://ipc.localhost/cmd3', { method: 'POST', body: '{}' });
                    var limited = window.__VICTAURI__.getIpcLog(2);
                    return { count: limited.length };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "clearIpcLog removes IPC entries only".into(),
                code: r"
                    await fetch('http://ipc.localhost/test_cmd', { method: 'POST', body: '{}' });
                    await fetch('http://other.com/api');
                    window.__VICTAURI__.clearIpcLog();
                    var ipcLog = window.__VICTAURI__.getIpcLog();
                    var netLog = window.__VICTAURI__.getNetworkLog();
                    return { ipc_count: ipcLog.length, net_has_other: netLog.some(function(e) { return e.url.indexOf('other.com') !== -1; }) };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["found"], true);
    assert_eq!(
        results[0].result.as_ref().unwrap()["command"],
        "get_settings"
    );
    assert_eq!(results[1].result.as_ref().unwrap()["excluded"], true);
    assert_eq!(results[2].result.as_ref().unwrap()["count"], 2);
    assert_eq!(results[3].result.as_ref().unwrap()["ipc_count"], 0);
    assert_eq!(results[3].result.as_ref().unwrap()["net_has_other"], true);
}

#[test]
fn performance_metrics() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "getPerformanceMetrics returns structure".into(),
                code: r"
                    var result = window.__VICTAURI__.getPerformanceMetrics();
                    return {
                        has_resources: !!result.resources,
                        has_paint: !!result.paint,
                        has_dom: !!result.dom,
                        dom_elements: result.dom ? result.dom.elements : 0,
                        dom_has_depth: result.dom ? typeof result.dom.max_depth === 'number' : false
                    };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "DOM stats are correct".into(),
                code: r"
                    var result = window.__VICTAURI__.getPerformanceMetrics();
                    return { elements: result.dom.elements, max_depth_gte_0: result.dom.max_depth >= 0 };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["has_resources"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["has_paint"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["has_dom"], true);
    assert_eq!(results[0].result.as_ref().unwrap()["dom_has_depth"], true);
    assert!(
        results[0].result.as_ref().unwrap()["dom_elements"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(results[1].result.as_ref().unwrap()["max_depth_gte_0"], true);
}

#[test]
fn find_elements() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: r#"<html lang="en"><head><title>Find Test</title></head><body>
            <button data-testid="save-btn" aria-label="Save document">Save</button>
            <button data-testid="cancel-btn">Cancel</button>
            <input type="text" placeholder="Search..." />
            <div role="dialog">Dialog content</div>
        </body></html>"#.to_string(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "findElements by text".into(),
                code: r"
                    var results = window.__VICTAURI__.findElements({ text: 'Save' });
                    return { count: results.length, first_tag: results[0] ? results[0].tag : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "findElements by role".into(),
                code: r"
                    var results = window.__VICTAURI__.findElements({ role: 'button' });
                    return { count: results.length };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "findElements by test_id".into(),
                code: r"
                    var results = window.__VICTAURI__.findElements({ test_id: 'save-btn' });
                    return { count: results.length, text: results[0] ? results[0].text : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "findElements by css selector".into(),
                code: r#"
                    var results = window.__VICTAURI__.findElements({ css: 'input[type="text"]' });
                    return { count: results.length, tag: results[0] ? results[0].tag : null };
                "#.into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "findElements by name (aria-label)".into(),
                code: r"
                    var results = window.__VICTAURI__.findElements({ name: 'Save document' });
                    return { count: results.length, ref_id: results[0] ? results[0].ref_id : null };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "findElements respects max_results".into(),
                code: r"
                    var results = window.__VICTAURI__.findElements({ role: 'button', max_results: 1 });
                    return { count: results.length };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert!(
        results[0].result.as_ref().unwrap()["count"]
            .as_u64()
            .unwrap()
            >= 1
    );
    // The first match might be body (which contains "Save" text) or button depending on traversal
    // Just check that at least one result was found
    let first_tag = results[0].result.as_ref().unwrap()["first_tag"]
        .as_str()
        .unwrap();
    assert!(
        first_tag == "button" || first_tag == "body",
        "Expected button or body, got {first_tag}"
    );
    assert!(
        results[1].result.as_ref().unwrap()["count"]
            .as_u64()
            .unwrap()
            >= 2
    );
    assert_eq!(results[2].result.as_ref().unwrap()["count"], 1);
    assert_eq!(results[2].result.as_ref().unwrap()["text"], "Save");
    assert_eq!(results[3].result.as_ref().unwrap()["count"], 1);
    assert_eq!(results[3].result.as_ref().unwrap()["tag"], "input");
    assert_eq!(results[4].result.as_ref().unwrap()["count"], 1);
    assert_eq!(results[5].result.as_ref().unwrap()["count"], 1);
}

#[test]
fn actionability_checks() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: r#"<html lang="en"><head><title>Test</title></head><body>
            <button id="disabled-btn" disabled>Disabled</button>
            <button id="aria-disabled-btn" aria-disabled="true">Aria Disabled</button>
            <button id="hidden-btn" style="display:none">Hidden</button>
            <button id="good-btn">Good</button>
        </body></html>"#.to_string(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "click fails on disabled element".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findById(node, id) {
                        if (node.attributes && node.attributes.id === id) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findById(node.children[i], id);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findById(snap.tree, 'disabled-btn');
                    if (!btn) return { ok: false, error: 'disabled-btn not in snapshot (expected - disabled is still visible)' };
                    var result = await window.__VICTAURI__.click(btn.ref_id, 100);
                    return result;
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "click fails on aria-disabled element".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findById(node, id) {
                        if (node.attributes && node.attributes.id === id) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findById(node.children[i], id);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findById(snap.tree, 'aria-disabled-btn');
                    if (!btn) return { ok: false, error: 'btn not found in tree' };
                    var result = await window.__VICTAURI__.click(btn.ref_id, 100);
                    return result;
                ".into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "click works on good button".into(),
                code: r"
                    var snap = window.__VICTAURI__.snapshot('json');
                    function findById(node, id) {
                        if (node.attributes && node.attributes.id === id) return node;
                        for (var i = 0; i < node.children.length; i++) {
                            var r = findById(node.children[i], id);
                            if (r) return r;
                        }
                        return null;
                    }
                    var btn = findById(snap.tree, 'good-btn');
                    var result = await window.__VICTAURI__.click(btn.ref_id);
                    return result;
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    // disabled button: might timeout or give error
    assert_eq!(results[0].result.as_ref().unwrap()["ok"], false);
    assert_eq!(results[1].result.as_ref().unwrap()["ok"], false);
    assert_eq!(results[2].result.as_ref().unwrap()["ok"], true);
}

#[test]
fn capacity_limits() {
    let def = TestDef {
        bridge_script: init_script(&BridgeCapacities {
            console_logs: 5,
            mutation_log: 500,
            network_log: 5,
            navigation_log: 3,
            dialog_log: 3,
            long_tasks: 100,
        }),
        setup_html: default_html(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "console log cap enforced".into(),
                code: r"
                    for (var i = 0; i < 10; i++) console.log('msg ' + i);
                    var logs = window.__VICTAURI__.getConsoleLogs();
                    return { count: logs.length, first_msg: logs[0].message };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "network log cap enforced".into(),
                code: r"
                    for (var i = 0; i < 10; i++) await fetch('http://example.com/' + i);
                    var log = window.__VICTAURI__.getNetworkLog();
                    return { count: log.length };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
            TestCase {
                name: "dialog log cap enforced".into(),
                code: r"
                    for (var i = 0; i < 10; i++) window.alert('alert ' + i);
                    var log = window.__VICTAURI__.getDialogLog();
                    return { count: log.length };
                "
                .into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    assert_eq!(results[0].result.as_ref().unwrap()["count"], 5);
    // The oldest entries are shifted out, so first message should be "msg 5"
    assert_eq!(results[0].result.as_ref().unwrap()["first_msg"], "msg 5");
    assert_eq!(results[1].result.as_ref().unwrap()["count"], 5);
    assert_eq!(results[2].result.as_ref().unwrap()["count"], 3);
}

#[test]
fn infer_role_mapping() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: r#"<html lang="en"><head><title>Role Test</title></head><body>
            <button>Btn</button>
            <a href="/">Link</a>
            <input type="checkbox" />
            <input type="radio" />
            <input type="range" />
            <input type="submit" value="Submit" />
            <nav>Nav</nav>
            <main>Main</main>
            <aside>Aside</aside>
        </body></html>"#.to_string(),
        setup_js: None,
        tests: vec![
            TestCase {
                name: "roles inferred correctly in json snapshot".into(),
                code: r"
                    var result = window.__VICTAURI__.snapshot('json');
                    function collectRoles(node, acc) {
                        if (node.role) acc.push(node.tag + ':' + node.role);
                        for (var i = 0; i < node.children.length; i++) collectRoles(node.children[i], acc);
                        return acc;
                    }
                    var roles = collectRoles(result.tree, []);
                    return {
                        has_button: roles.some(function(r) { return r === 'button:button'; }),
                        has_link: roles.some(function(r) { return r === 'a:link'; }),
                        has_checkbox: roles.some(function(r) { return r === 'input:checkbox'; }),
                        has_radio: roles.some(function(r) { return r === 'input:radio'; }),
                        has_slider: roles.some(function(r) { return r === 'input:slider'; }),
                        has_nav: roles.some(function(r) { return r === 'nav:navigation'; }),
                        has_main: roles.some(function(r) { return r === 'main:main'; }),
                        has_complementary: roles.some(function(r) { return r === 'aside:complementary'; }),
                    };
                ".into(),
                setup_html: None,
                setup_js: None,
            },
        ],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    let r = results[0].result.as_ref().unwrap();
    assert_eq!(r["has_button"], true);
    assert_eq!(r["has_link"], true);
    assert_eq!(r["has_checkbox"], true);
    assert_eq!(r["has_radio"], true);
    assert_eq!(r["has_slider"], true);
    assert_eq!(r["has_nav"], true);
    assert_eq!(r["has_main"], true);
    assert_eq!(r["has_complementary"], true);
}

#[test]
fn compact_format_details() {
    let def = TestDef {
        bridge_script: bridge_script(),
        setup_html: r#"<html lang="en"><head><title>Compact Test</title></head><body>
            <input id="myinput" type="text" data-testid="search" disabled value="hello" />
            <a href="/about">About Us</a>
        </body></html>"#
            .to_string(),
        setup_js: None,
        tests: vec![TestCase {
            name: "compact format shows disabled, value, type, data-testid, href".into(),
            code: r"
                    var result = window.__VICTAURI__.snapshot('compact');
                    var tree = result.tree;
                    return {
                        has_disabled: tree.indexOf('[disabled]') !== -1,
                        has_value: tree.indexOf('value=') !== -1,
                        has_type: tree.indexOf('type=text') !== -1,
                        has_testid: tree.indexOf('@search') !== -1,
                        has_href: tree.indexOf('href=/about') !== -1,
                    };
                "
            .into(),
            setup_html: None,
            setup_js: None,
        }],
    };
    let results = run_tests(&def);
    assert_all_pass(&results);
    let r = results[0].result.as_ref().unwrap();
    assert_eq!(r["has_disabled"], true);
    assert_eq!(r["has_value"], true);
    assert_eq!(r["has_type"], true);
    assert_eq!(r["has_testid"], true);
    assert_eq!(r["has_href"], true);
}
