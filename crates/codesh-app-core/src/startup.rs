use crate::{AppError, AppState, PaneContent, PaneId};

impl AppState {
    pub fn startup_command(&self, pane: PaneId) -> Option<&str> {
        self.startup_commands.get(&pane).map(String::as_str)
    }

    pub fn set_startup_command(&mut self, pane: PaneId, command: &str) -> Result<(), AppError> {
        if !matches!(self.pane_mut(pane)?.content, PaneContent::Terminal { .. }) {
            return Err(AppError::NotTerminal(pane));
        }
        let command = command.trim();
        if !command.is_empty() {
            self.startup_commands.insert(pane, command.to_owned());
        }
        Ok(())
    }

    pub fn take_startup_command(&mut self, pane: PaneId) -> Option<String> {
        self.startup_commands.remove(&pane)
    }
}
