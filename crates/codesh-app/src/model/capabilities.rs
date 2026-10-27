use codesh_core::capability::{Capabilities, Capability, Feature};
use gpui::Context;

use super::AppModel;

fn backend(feature: Feature) -> Capability {
    match feature {
        Feature::Voice => codesh_ui::voice::CAPABILITY,
        Feature::Notifications => codesh_ui::notifications::CAPABILITY,
        Feature::CommandLineTool => crate::cli_install::CAPABILITY,
        Feature::Updates => crate::updater::CAPABILITY,
        Feature::AppHiding => Capability::of(cfg!(target_os = "macos")),
    }
}

/// What this build has backends for. Services refine the table as they start.
pub(crate) fn backends() -> Capabilities {
    let mut capabilities = Capabilities::default();
    for feature in Feature::ALL {
        capabilities.set(feature, backend(feature));
    }
    capabilities
}

impl AppModel {
    pub(crate) fn supports(&self, feature: Feature) -> bool {
        self.capabilities.supports(feature)
    }

    /// Whether `feature` can run. Otherwise reports why, before any work starts.
    pub(crate) fn require(&mut self, feature: Feature, cx: &mut Context<Self>) -> bool {
        let Some(reason) = self.capabilities.reason(feature) else {
            return true;
        };
        self.fail(reason.into_owned(), cx);
        false
    }

    /// Leaves an app-wide command to its own handler where `feature` can run.
    pub(crate) fn forward(&mut self, feature: Feature, cx: &mut Context<Self>) {
        if self.require(feature, cx) {
            cx.propagate();
        }
    }
}
