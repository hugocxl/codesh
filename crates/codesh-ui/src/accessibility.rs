//! Native accessibility preferences shared by desktop surfaces.

/// Respect macOS display accessibility settings before enabling vibrancy.
#[cfg(target_os = "macos")]
pub fn transparency_allowed() -> bool {
    let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
    !workspace.accessibilityDisplayShouldReduceTransparency()
        && !workspace.accessibilityDisplayShouldIncreaseContrast()
}
