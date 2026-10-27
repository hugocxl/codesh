#![cfg(target_os = "macos")]

use codesh_app_core::browser::automation::{
    CANCEL_ELEMENT_PICKER, ELEMENT_HTML_LIMIT, element_picker_script,
};
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn picker_restores_real_dom_styles_and_bounds_unicode_output()
-> Result<(), Box<dyn std::error::Error>> {
    let script = tempfile::NamedTempFile::new()?;
    std::fs::write(
        script.path(),
        format!(
            "const pickerBody = {};\nconst cancelBody = {};\n{}",
            json!(element_picker_script("#12abef")),
            json!(CANCEL_ELEMENT_PICKER),
            include_str!("fixtures/browser_picker.js"),
        ),
    )?;
    let output = Command::new("swift")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/browser_picker.swift"
        ))
        .arg(script.path())
        .output()?;
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let result: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(result["passed"], true);
    assert_eq!(
        result["html"]
            .as_str()
            .ok_or("HTML result missing")?
            .chars()
            .count(),
        ELEMENT_HTML_LIMIT
    );
    assert_eq!(result["textTail"], "x😀");
    Ok(())
}
