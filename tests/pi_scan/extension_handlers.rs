//! Regression contracts for the embedded restricted-tool extension's handler calling convention.

use pacsea::pi_agent::process::EMBEDDED_EXTENSION_SOURCE;
use std::process::Command;

/// What: Require every embedded tool handler to take the tool-call id before its parameters.
///
/// Inputs:
/// - The compiled `pacsea-scan-tools.ts` source.
///
/// Output:
/// - Asserts four `execute(` declarations, each starting with a `_toolCallId: string` parameter.
///
/// Details:
/// - Pi calls `execute(toolCallId, params, signal, onUpdate, ctx)`. A handler that reads its
///   first argument as params sees a string and rejects every call. This check is hermetic.
#[test]
fn embedded_tool_handlers_take_tool_call_id_first() {
    let declarations: Vec<&str> = EMBEDDED_EXTENSION_SOURCE
        .match_indices("async execute(")
        .map(|(index, needle)| &EMBEDDED_EXTENSION_SOURCE[index + needle.len()..])
        .collect();
    assert_eq!(declarations.len(), 4, "expected exactly four tool handlers");
    for rest in declarations {
        assert!(
            rest.trim_start().starts_with("_toolCallId: string,"),
            "handler does not take the tool-call id first: {}",
            rest.lines().next().unwrap_or_default()
        );
    }
}

/// Minimal `typebox` stand-in so the extension can be imported without Pi installed.
const TYPEBOX_STUB: &str = r"
const make = (kind) => (...args) => ({ kind, args });
export const Type = {
  Object: make('object'),
  String: make('string'),
  Integer: make('integer'),
  Boolean: make('boolean'),
  Optional: make('optional'),
};
";

/// Driver that registers the extension and calls each handler the way Pi does.
const DRIVER: &str = r#"
import register from "./pacsea-scan-tools.ts";
const tools = new Map();
register({
  registerTool: (tool) => tools.set(tool.name, tool),
  registerCommand: () => {},
  getActiveTools: () => [],
});
const execute = (name, params) =>
  tools.get(name).execute("call-1", params, undefined, undefined, {});
const call = async (name, params) => JSON.parse((await execute(name, params)).content[0].text);
const flag = async (name, params) => (await execute(name, params)).isError ?? null;
const out = {
  readIsError: await flag("pacsea_scan_read", { snapshot: "snap", relative_path: "PKGBUILD" }),
  rejected: await call("pacsea_scan_read", { snapshot: "snap", relative_path: "../escape" }),
  rejectedIsError: await flag("pacsea_scan_read", { snapshot: "snap", relative_path: "../escape" }),
  unknownSnapshotIsError: await flag("pacsea_scan_ls", { snapshot: "missing" }),
  read: await call("pacsea_scan_read", { snapshot: "snap", relative_path: "PKGBUILD" }),
  grep: await call("pacsea_scan_grep", { snapshot: "snap", literal: "pkgname" }),
  find: await call("pacsea_scan_find", { snapshot: "snap", glob: "**" }),
  ls: await call("pacsea_scan_ls", { snapshot: "snap" }),
};
process.stdout.write(JSON.stringify(out));
"#;

/// What: Run the embedded handlers in Node with Pi's `(toolCallId, params, ...)` call shape.
///
/// Inputs:
/// - A temporary directory holding the asset, a `typebox` stub, a descriptor, and a snapshot.
///
/// Output:
/// - Asserts read, grep, find, and ls all return snapshot content rather than an error, and
///   that a rejected request is reported with `isError: true` while a success is not.
///
/// Details:
/// - Skips with a message when `node` is not installed, since Node is not a build dependency.
///   Node strips the TypeScript annotations natively; no Pi package or network is used.
#[test]
fn embedded_tool_handlers_answer_pi_shaped_calls_under_node() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipping: node is not installed");
        return;
    }
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    let snapshot = root.join("snapshot");
    std::fs::create_dir_all(&snapshot).expect("snapshot dir");
    std::fs::write(snapshot.join("PKGBUILD"), "pkgname=demo\n").expect("snapshot file");
    let typebox = root.join("node_modules/typebox");
    std::fs::create_dir_all(&typebox).expect("typebox dir");
    std::fs::write(
        typebox.join("package.json"),
        r#"{"name":"typebox","type":"module","main":"index.js"}"#,
    )
    .expect("typebox package");
    std::fs::write(typebox.join("index.js"), TYPEBOX_STUB).expect("typebox stub");
    std::fs::write(root.join("package.json"), r#"{"type":"module"}"#).expect("package.json");
    std::fs::write(root.join("pacsea-scan-tools.ts"), EMBEDDED_EXTENSION_SOURCE).expect("asset");
    let descriptor = serde_json::json!({ "snap": snapshot.to_string_lossy() });
    std::fs::write(
        root.join("pacsea-scan-descriptor.json"),
        descriptor.to_string(),
    )
    .expect("descriptor");
    std::fs::write(root.join("driver.mjs"), DRIVER).expect("driver");

    let output = Command::new("node")
        .arg("driver.mjs")
        .current_dir(root)
        .output()
        .expect("run node");
    assert!(
        output.status.success(),
        "node driver failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let results: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("driver JSON output");
    assert_eq!(results["read"]["text"], "pkgname=demo\n", "{results}");
    assert_eq!(
        results["grep"]["matches"][0]["path"], "PKGBUILD",
        "{results}"
    );
    assert_eq!(
        results["find"]["entries"][0]["path"], "PKGBUILD",
        "{results}"
    );
    assert_eq!(results["ls"]["entries"][0]["path"], "PKGBUILD", "{results}");
    // Pi copies `isError` into `tool_execution_end`; the scan engine counts only explicit
    // `false` as a successful read, so rejections must carry `true`.
    assert_ne!(results["readIsError"], true, "{results}");
    assert!(results["rejected"]["error"].is_string(), "{results}");
    assert_eq!(results["rejectedIsError"], true, "{results}");
    assert_eq!(results["unknownSnapshotIsError"], true, "{results}");
}
