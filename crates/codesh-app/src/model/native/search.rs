use codesh_app_core::ProjectId;
use codesh_protocol::ExecResult;
use gpui::{AppContext, Context, Focusable, Window};

use crate::model::AppModel;
use crate::views::find_in_files::{FindEvent, FindInFiles, parse_rg_json};
use crate::views::overlays::Overlay;
use crate::views::quick_open::{QuickOpen, QuickOpenEvent};

/// A project's files for Go to File, kept until its files change.
pub(crate) struct FileList {
    files: Vec<String>,
    /// Files were added or removed since; the list is fetched again on the
    /// next use, and shown meanwhile.
    pub(crate) stale: bool,
    loading: bool,
}

fn argv(arguments: &[&str]) -> Vec<String> {
    arguments
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect()
}

fn listed_files(result: &Result<ExecResult, String>) -> Option<Vec<String>> {
    let result = result
        .as_ref()
        .ok()
        .filter(|result| result.exit_code == 0 && !result.timed_out && !result.cancelled)?;
    Some(
        result
            .stdout
            .lines()
            .map(|line| line.trim_start_matches("./"))
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// The one-based character column of ripgrep's one-based byte `column` in
/// `line`.
fn character_column(line: &str, column: u32) -> u32 {
    let bytes = usize::try_from(column.saturating_sub(1)).unwrap_or(usize::MAX);
    line.get(..bytes)
        .and_then(|prefix| u32::try_from(prefix.chars().count()).ok())
        .map_or(1, |count| count + 1)
}

/// The ripgrep command line for a Find in Files query.
pub(crate) fn search_argv(query: &str, case_sensitive: bool, regex: bool) -> Vec<String> {
    let mut arguments = argv(&[
        "rg",
        "--json",
        "--hidden",
        "-g",
        "!.git",
        "--max-count",
        "200",
    ]);
    if !case_sensitive {
        arguments.push("-i".into());
    }
    if !regex {
        arguments.push("-F".into());
    }
    arguments.push("-e".into());
    arguments.push(query.into());
    arguments
}

impl AppModel {
    /// Opens Go to File over the current project, or closes it.
    pub(crate) fn toggle_quick_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::QuickOpen(_))) {
            self.dismiss_overlay(cx);
            return;
        }
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let project = self.state.current_project().id;
        let (theme, metrics) = (self.theme.clone(), self.metrics);
        let picker = cx.new(|cx| QuickOpen::new(&theme, metrics, cx));
        self.overlay_subscription =
            Some(
                cx.subscribe_in(&picker, window, move |model, _, event, _, cx| match event {
                    QuickOpenEvent::Open { path } => {
                        model.dismiss_overlay(cx);
                        model.open_project_file(project, path, None, cx);
                    }
                    QuickOpenEvent::Dismissed => model.dismiss_overlay(cx),
                }),
            );
        picker.focus_handle(cx).focus(window);
        self.overlay = Some(Overlay::QuickOpen(picker));
        self.focus_requested = false;
        self.load_file_list(project, cx);
        cx.notify();
    }

    fn show_file_list(&self, project: ProjectId, cx: &mut Context<Self>) {
        let (Some(Overlay::QuickOpen(picker)), Some(list)) =
            (&self.overlay, self.native.file_lists.get(&project))
        else {
            return;
        };
        if project == self.state.current_project().id {
            let files = list.files.clone();
            picker.update(cx, |picker, cx| picker.set_files(files, cx));
        }
    }

    /// Shows `project`'s files, listing them on its server when they are not
    /// known or have changed: with ripgrep, or `git ls-files` without it.
    fn load_file_list(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.show_file_list(project, cx);
        if self
            .native
            .file_lists
            .get(&project)
            .is_some_and(|list| list.loading || !list.stale)
        {
            return;
        }
        let list = self
            .native
            .file_lists
            .entry(project)
            .or_insert_with(|| FileList {
                files: Vec::new(),
                stale: true,
                loading: false,
            });
        list.loading = true;
        let rg = argv(&["rg", "--files", "--hidden", "-g", "!.git"]);
        self.exec_request(project, rg, cx, move |model, result, cx| {
            if let Some(files) = listed_files(&result) {
                model.file_list_loaded(project, files, cx);
                return;
            }
            let git = argv(&[
                "git",
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
            ]);
            model.exec_request(project, git, cx, move |model, result, cx| {
                let files = listed_files(&result).unwrap_or_default();
                model.file_list_loaded(project, files, cx);
            });
        });
    }

    fn file_list_loaded(&mut self, project: ProjectId, files: Vec<String>, cx: &mut Context<Self>) {
        let Some(list) = self.native.file_lists.get_mut(&project) else {
            return;
        };
        list.loading = false;
        list.stale = false;
        list.files = files;
        self.show_file_list(project, cx);
    }

    /// Opens Find in Files over the current project, or closes it.
    pub(crate) fn toggle_find_in_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::FindInFiles(_))) {
            self.dismiss_overlay(cx);
            return;
        }
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let project = self.state.current_project().id;
        let (theme, metrics) = (self.theme.clone(), self.metrics);
        let picker = cx.new(|cx| FindInFiles::new(&theme, metrics, cx));
        self.overlay_subscription =
            Some(
                cx.subscribe_in(&picker, window, move |model, _, event, _, cx| match event {
                    FindEvent::Search {
                        query,
                        case_sensitive,
                        regex,
                    } => model.search_files(project, query, *case_sensitive, *regex, cx),
                    FindEvent::Open { path, line } => {
                        let column = model
                            .native
                            .find_results
                            .iter()
                            .find(|found| found.path == *path && found.line == *line)
                            .map_or(1, |found| character_column(&found.preview, found.column));
                        model.dismiss_overlay(cx);
                        model.open_project_file(project, path, Some((*line, column)), cx);
                    }
                    FindEvent::Dismissed => model.dismiss_overlay(cx),
                }),
            );
        picker.focus_handle(cx).focus(window);
        self.native.find_results.clear();
        self.overlay = Some(Overlay::FindInFiles(picker));
        self.focus_requested = false;
        cx.notify();
    }

    fn search_files(
        &mut self,
        project: ProjectId,
        query: &str,
        case_sensitive: bool,
        regex: bool,
        cx: &mut Context<Self>,
    ) {
        self.native.search += 1;
        let search = self.native.search;
        if query.is_empty() {
            self.found(search, Vec::new(), cx);
            return;
        }
        let arguments = search_argv(query, case_sensitive, regex);
        self.exec_request(project, arguments, cx, move |model, result, cx| {
            let found = result
                .map(|result| parse_rg_json(&result.stdout))
                .unwrap_or_default();
            model.found(search, found, cx);
        });
    }

    fn found(
        &mut self,
        search: u64,
        found: Vec<crate::views::find_in_files::FindMatch>,
        cx: &mut Context<Self>,
    ) {
        if search != self.native.search {
            return;
        }
        let Some(Overlay::FindInFiles(picker)) = &self.overlay else {
            return;
        };
        self.native.find_results.clone_from(&found);
        picker.update(cx, |picker, cx| picker.set_results(found, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_in_files_flags_and_byte_columns() {
        assert_eq!(
            search_argv("a.b", false, false)[7..],
            ["-i", "-F", "-e", "a.b"].map(str::to_owned)
        );
        assert_eq!(
            search_argv("-x", true, true)[7..],
            ["-e", "-x"].map(str::to_owned)
        );
        assert_eq!(character_column("héllo", 4), 3);
        assert_eq!(character_column("abc", 99), 1);
    }
}
