//! Files and searches behind the palette's Go to File and Find in Files pages.

use codesh_app_core::ProjectId;
use codesh_protocol::ExecResult;
use gpui::Context;

use crate::model::AppModel;

/// A project's files for Go to File, kept until its files change.
pub(crate) struct FileList {
    files: Vec<String>,
    /// Files were added or removed since; the list is fetched again on the
    /// next use, and shown meanwhile.
    pub(crate) stale: bool,
    loading: bool,
}

impl FileList {
    pub(crate) fn files(&self) -> &[String] {
        &self.files
    }

    /// Whether the files are being listed, the first time or again.
    pub(crate) fn loading(&self) -> bool {
        self.loading
    }
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
pub(crate) fn character_column(line: &str, column: u32) -> u32 {
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

/// One ripgrep match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FindMatch {
    pub(crate) path: String,
    pub(crate) line: u32,
    /// One-based byte column, as reported by ripgrep.
    pub(crate) column: u32,
    pub(crate) preview: String,
}

impl FindMatch {
    /// The one-based character column to use when opening this match in an editor.
    pub(crate) fn character_column(&self) -> u32 {
        character_column(&self.preview, self.column)
    }
}

// Ripgrep encodes non-UTF-8 paths and lines in a base64 `bytes` field.
fn field_bytes(value: &serde_json::Value) -> Option<Vec<u8>> {
    if let Some(text) = value.get("text").and_then(serde_json::Value::as_str) {
        return Some(text.as_bytes().to_vec());
    }
    let encoded = value.get("bytes")?.as_str()?;
    let mut output = Vec::with_capacity(encoded.len() * 3 / 4);
    let mut bits = 0_u32;
    let mut count = 0;
    for byte in encoded.bytes().take_while(|byte| *byte != b'=') {
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(digit);
        count += 6;
        if count >= 8 {
            count -= 8;
            output.push(u8::try_from((bits >> count) & 255).ok()?);
        }
    }
    Some(output)
}

pub(crate) fn parse_rg_json(output: &str) -> Vec<FindMatch> {
    let mut matches = Vec::new();
    for record in output.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(record) else {
            continue;
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some("match") {
            continue;
        }
        let Some(data) = value.get("data") else {
            continue;
        };
        let (Some(path), Some(lines), Some(line), Some(submatches)) = (
            data.get("path").and_then(field_bytes),
            data.get("lines").and_then(field_bytes),
            data.get("line_number")
                .and_then(serde_json::Value::as_u64)
                .and_then(|n| u32::try_from(n).ok()),
            data.get("submatches").and_then(serde_json::Value::as_array),
        ) else {
            continue;
        };
        let mut line_starts = vec![0];
        line_starts.extend(
            lines
                .iter()
                .enumerate()
                .filter_map(|(index, &byte)| (byte == b'\n').then_some(index + 1)),
        );
        for submatch in submatches {
            let Some(start) = submatch
                .get("start")
                .and_then(serde_json::Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
                .filter(|&n| n <= lines.len())
            else {
                continue;
            };
            let extra_lines = line_starts.partition_point(|&offset| offset <= start) - 1;
            let line_start = line_starts[extra_lines];
            let line_end = line_starts
                .get(extra_lines + 1)
                .map_or(lines.len(), |n| n - 1);
            let (Some(line), Some(column)) = (
                u32::try_from(extra_lines)
                    .ok()
                    .and_then(|n| line.checked_add(n)),
                u32::try_from(start - line_start)
                    .ok()
                    .and_then(|n| n.checked_add(1)),
            ) else {
                continue;
            };
            let preview_bytes = &lines[line_start..line_end];
            let preview_bytes = preview_bytes.strip_suffix(b"\r").unwrap_or(preview_bytes);
            matches.push(FindMatch {
                path: String::from_utf8_lossy(&path).into_owned(),
                line,
                column,
                preview: String::from_utf8_lossy(preview_bytes).into_owned(),
            });
        }
    }
    matches
}

impl AppModel {
    /// `project`'s files, as last listed.
    pub(crate) fn file_list(&self, project: ProjectId) -> Option<&FileList> {
        self.native.file_lists.get(&project)
    }

    /// The last Find in Files results.
    pub(crate) fn find_results(&self) -> &[FindMatch] {
        &self.native.find_results
    }

    /// Whether a Find in Files search has not answered yet.
    pub(crate) fn search_in_flight(&self) -> bool {
        self.native.searched != self.native.search
    }

    /// Lists `project`'s files on its server when they are not known or have
    /// changed: with ripgrep, or `git ls-files` without it.
    pub(crate) fn load_file_list(&mut self, project: ProjectId, cx: &mut Context<Self>) {
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
        self.refresh_files_page(cx);
        self.refresh_root_page(cx);
    }

    pub(crate) fn search_files(
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

    fn found(&mut self, search: u64, found: Vec<FindMatch>, cx: &mut Context<Self>) {
        if search != self.native.search {
            return;
        }
        self.native.searched = search;
        self.native.find_results = found;
        self.refresh_find_page(cx);
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

    #[test]
    fn find_in_files_parses_multiple_submatches_and_multiline_offsets() {
        let output = concat!(
            "{\"type\":\"begin\"}\n",
            "garbage\n",
            r#"{"type":"match","data":{"path":{"text":"src/a.rs"},"lines":{"text":"foo foo\nbar foo\r\n"},"line_number":7,"submatches":[{"start":0},{"start":4},{"start":12},{"start":900}]}}"#
        );
        let found = parse_rg_json(output);
        assert_eq!(
            found
                .iter()
                .map(|m| (m.line, m.column, m.preview.as_str()))
                .collect::<Vec<_>>(),
            [(7, 1, "foo foo"), (7, 5, "foo foo"), (8, 5, "bar foo")]
        );
        assert!(found.iter().all(|m| m.path == "src/a.rs"));
    }

    #[test]
    fn find_in_files_tolerates_binary_fields_and_incomplete_records() {
        let found = parse_rg_json(
            r#"{"type":"match","data":{"path":{"bytes":"Yf8="},"lines":{"bytes":"/2ZvbyA="},"line_number":1,"submatches":[{"start":1}]}}
{"type":"match","data":{}}
{"type":"summary"}"#,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "a�");
        assert_eq!(found[0].preview, "�foo ");
    }
}
