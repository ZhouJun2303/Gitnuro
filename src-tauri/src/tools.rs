//! External diff and merge tools taken from Fork's settings.
//!
//! A built-in Fork type is kept only when the file on disk is that program.
//! A path that is missing, or a Beyond Compare entry that points at another executable, is dropped.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTool {
    pub name: String,
    pub path: String,
    pub args: String,
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTools {
    pub merge: Vec<ExternalTool>,
    pub diff: Vec<ExternalTool>,
}

/// A known tool that supports both modes, used by the settings import.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonTool {
    pub name: String,
    pub path: String,
    pub diff_args: String,
    pub merge_args: String,
}

#[derive(Clone, Copy)]
enum Kind {
    Merge,
    Diff,
}

struct RawTool {
    kind_type: String,
    name: String,
    path: String,
    arguments: String,
    primary: bool,
}

pub fn program_exists(path: &str) -> bool {
    let path = path.trim();
    !path.is_empty() && Path::new(path).is_file()
}

pub fn installed() -> ExternalTools {
    let Some(path) = fork_settings_path() else {
        return ExternalTools { merge: Vec::new(), diff: Vec::new() };
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return ExternalTools { merge: Vec::new(), diff: Vec::new() };
    };
    tools_from_fork_json(&text, program_exists)
}

pub fn fork_settings_file() -> Option<std::path::PathBuf> {
    fork_settings_path()
}

/// Tools named in a Fork settings file, including programs that are not installed yet.
pub fn tools_in_fork_text(text: &str) -> ExternalTools {
    tools_from_fork_json(text, |_| true)
}

/// Tools from well-known install folders. Only programs present on disk are returned.
pub fn common_tools() -> Vec<CommonTool> {
    let mut tools: Vec<CommonTool> = Vec::new();
    for preset in common_presets() {
        if preset.path.trim().is_empty() || !program_exists(&preset.path) {
            continue;
        }
        if tools.iter().any(|item| same_path(&item.path, &preset.path)) {
            continue;
        }
        tools.push(preset);
    }
    tools
}

fn common_presets() -> Vec<CommonTool> {
    let program = |folder: &str, file: &str| {
        std::env::var("ProgramFiles")
            .ok()
            .map(|base| Path::new(&base).join(folder).join(file).display().to_string())
            .unwrap_or_default()
    };
    let program_x86 = |folder: &str, file: &str| {
        std::env::var("ProgramFiles(x86)")
            .ok()
            .map(|base| Path::new(&base).join(folder).join(file).display().to_string())
            .unwrap_or_default()
    };
    let local = |folder: &str, file: &str| {
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|base| Path::new(&base).join(folder).join(file).display().to_string())
            .unwrap_or_default()
    };
    let mut rows = vec![
        CommonTool {
            name: "Beyond Compare".into(),
            path: program("Beyond Compare 5", "BCompare.exe"),
            diff_args: "{left} {right}".into(),
            merge_args: "{ours} {theirs} {base} {working}".into(),
        },
        CommonTool {
            name: "Beyond Compare".into(),
            path: program("Beyond Compare 4", "BCompare.exe"),
            diff_args: "{left} {right}".into(),
            merge_args: "{ours} {theirs} {base} {working}".into(),
        },
        CommonTool {
            name: "WinMerge".into(),
            path: program("WinMerge", "WinMergeU.exe"),
            diff_args: "{left} {right}".into(),
            merge_args: "-e -ub {ours} {theirs} {working}".into(),
        },
        CommonTool {
            name: "KDiff3".into(),
            path: program("KDiff3", "kdiff3.exe"),
            diff_args: "{left} {right}".into(),
            merge_args: "{base} {ours} {theirs} -o {working}".into(),
        },
        CommonTool {
            name: "Meld".into(),
            path: program("Meld", "Meld.exe"),
            diff_args: "{left} {right}".into(),
            merge_args: "{ours} {base} {theirs} -o {working}".into(),
        },
        CommonTool {
            name: "P4Merge".into(),
            path: program("Perforce", "p4merge.exe"),
            diff_args: "{left} {right}".into(),
            merge_args: "{base} {ours} {theirs} {working}".into(),
        },
        CommonTool {
            name: "TortoiseGitMerge".into(),
            path: program("TortoiseGit", "bin\\TortoiseGitMerge.exe"),
            diff_args: "{left} {right}".into(),
            merge_args: "-mine:{ours} -base:{base} -theirs:{theirs} -merged:{working}".into(),
        },
        CommonTool {
            name: "VS Code".into(),
            path: local("Programs\\Microsoft VS Code", "Code.exe"),
            diff_args: "--diff \"{left}\" \"{right}\"".into(),
            merge_args: String::new(),
        },
        CommonTool {
            name: "VS Code".into(),
            path: program("Microsoft VS Code", "Code.exe"),
            diff_args: "--diff \"{left}\" \"{right}\"".into(),
            merge_args: String::new(),
        },
    ];
    for (folder, file) in [("WinMerge", "WinMergeU.exe"), ("KDiff3", "kdiff3.exe"), ("Meld", "Meld.exe")] {
        rows.push(CommonTool {
            name: folder.into(),
            path: program_x86(folder, file),
            diff_args: "{left} {right}".into(),
            merge_args: match folder {
                "KDiff3" => "{base} {ours} {theirs} -o {working}".into(),
                "Meld" => "{ours} {base} {theirs} -o {working}".into(),
                _ => "-e -ub {ours} {theirs} {working}".into(),
            },
        });
    }
    rows
}

pub fn normalize_extension(value: &str) -> String {
    let text = value.trim();
    if text == "*" {
        return "*".into();
    }
    let text = text.strip_prefix("*.").unwrap_or(text);
    text.trim_start_matches('.').to_ascii_lowercase()
}

fn tool_is_generic(tool: &crate::settings::ToolRecord) -> bool {
    tool.fallback || tool.extensions.is_empty() || tool.extensions.iter().all(|item| normalize_extension(item) == "*")
}

fn tool_matches(tool: &crate::settings::ToolRecord, file: &str) -> bool {
    let leaf = Path::new(file).file_name().and_then(|name| name.to_str()).unwrap_or(file).to_ascii_lowercase();
    tool.extensions.iter().any(|item| {
        let value = normalize_extension(item);
        value == "*" || (!value.is_empty() && (leaf == value || leaf.ends_with(&format!(".{value}"))))
    })
}

/// The first tool for this file: an exact extension match wins, then the
/// fallback tool, then an all-files tool, then the top of the list.
pub fn pick_tool<'a>(tools: &'a [crate::settings::ToolRecord], file: &str) -> Option<&'a crate::settings::ToolRecord> {
    tools
        .iter()
        .find(|tool| tool_matches(tool, file))
        .or_else(|| tools.iter().find(|tool| tool.fallback))
        .or_else(|| tools.iter().find(|tool| tool_is_generic(tool)))
        .or_else(|| tools.first())
}

fn fork_settings_path() -> Option<std::path::PathBuf> {
    let base = std::env::var("LOCALAPPDATA").ok()?;
    Some(Path::new(&base).join("Fork").join("settings.json"))
}

fn tools_from_fork_json(text: &str, exists: impl Fn(&str) -> bool) -> ExternalTools {
    let Ok(json) = serde_json::from_str::<Value>(text) else {
        return ExternalTools { merge: Vec::new(), diff: Vec::new() };
    };
    ExternalTools {
        merge: collect(&json, Kind::Merge, "ExternalMergeTools", "MergeTool", &exists),
        diff: collect(&json, Kind::Diff, "ExternalDiffTools", "ExternalDiffTool", &exists),
    }
}

fn collect(json: &Value, kind: Kind, list_key: &str, active_key: &str, exists: &impl Fn(&str) -> bool) -> Vec<ExternalTool> {
    let mut raw = Vec::new();
    if let Some(items) = json.get(list_key).and_then(Value::as_array) {
        for item in items {
            push_tool(&mut raw, item, false, exists);
        }
    }
    if let Some(active) = json.get(active_key).filter(|value| !value.is_null()) {
        let path = text_of(active, &["Path", "ApplicationPath"]);
        push_tool(&mut raw, active, true, exists);
        if exists(path) {
            mark_primary(&mut raw, path);
        }
    }
    raw.iter().filter_map(|item| finish(kind, item)).collect()
}

fn push_tool(list: &mut Vec<RawTool>, value: &Value, primary: bool, exists: &impl Fn(&str) -> bool) {
    let path = text_of(value, &["Path", "ApplicationPath"]).trim().to_string();
    if path.is_empty() || !exists(&path) {
        return;
    }
    let kind_type = text_of(value, &["Type"]).trim().to_string();
    let name = text_of(value, &["Name"]).trim().to_string();
    let arguments = text_of(value, &["Arguments"]).to_string();
    let primary = primary || value.get("IsPrimary").and_then(Value::as_bool).unwrap_or(false);
    if let Some(existing) = list.iter_mut().find(|item| same_path(&item.path, &path)) {
        if existing.arguments.trim().is_empty() && !arguments.trim().is_empty() {
            existing.arguments = arguments;
        }
        if existing.name.is_empty() && !name.is_empty() {
            existing.name = name;
        }
        if existing.kind_type != "Custom" && kind_type == "Custom" {
            existing.kind_type = kind_type;
        }
        if primary {
            existing.primary = true;
        }
        return;
    }
    list.push(RawTool { kind_type, name, path, arguments, primary });
}

fn mark_primary(list: &mut [RawTool], path: &str) {
    if !list.iter().any(|item| same_path(&item.path, path)) {
        return;
    }
    for item in list.iter_mut() {
        item.primary = same_path(&item.path, path);
    }
}

fn finish(kind: Kind, raw: &RawTool) -> Option<ExternalTool> {
    let args = if !raw.arguments.trim().is_empty() {
        translate(kind, &raw.arguments)
    } else if let Some(builtin) = builtin_args(kind, &raw.kind_type, &raw.path) {
        builtin
    } else if raw.kind_type.is_empty() || raw.kind_type == "Custom" {
        String::new()
    } else {
        return None;
    };
    Some(ExternalTool {
        name: tool_name(raw),
        path: raw.path.clone(),
        args,
        primary: raw.primary,
    })
}

fn tool_name(raw: &RawTool) -> String {
    if builtin_matches(&raw.kind_type, &raw.path) {
        if let Some(label) = type_label(&raw.kind_type) {
            return label.to_string();
        }
    }
    if !raw.name.trim().is_empty() {
        return raw.name.trim().to_string();
    }
    Path::new(&raw.path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Tool")
        .to_string()
}

fn text_of<'a>(value: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter().find_map(|key| value.get(*key).and_then(Value::as_str)).unwrap_or("")
}

fn same_path(left: &str, right: &str) -> bool {
    let norm = |value: &str| value.trim().trim_end_matches(['\\', '/']).replace('/', "\\").to_ascii_lowercase();
    !left.trim().is_empty() && norm(left) == norm(right)
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().and_then(|name| name.to_str()).unwrap_or("").to_ascii_lowercase()
}

fn type_label(kind_type: &str) -> Option<&'static str> {
    match kind_type {
        "BeyondCompare" | "BeyondCompare3" | "BeyondCompare4" | "BeyondCompare5" => Some("Beyond Compare"),
        "AraxisMerge" | "Araxis" => Some("Araxis Merge"),
        "KDiff3" => Some("KDiff3"),
        "Meld" => Some("Meld"),
        "P4Merge" => Some("P4Merge"),
        "WinMerge" => Some("WinMerge"),
        _ => None,
    }
}

fn builtin_matches(kind_type: &str, path: &str) -> bool {
    let file = file_name(path);
    let folder = path.replace('/', "\\").to_ascii_lowercase();
    match kind_type {
        "BeyondCompare" | "BeyondCompare3" | "BeyondCompare4" | "BeyondCompare5" => file == "bcomp.exe" || file == "bcompare.exe",
        "AraxisMerge" | "Araxis" => folder.contains("araxis") && (file == "merge.exe" || file == "compare.exe"),
        "KDiff3" => file == "kdiff3.exe",
        "Meld" => file == "meld.exe",
        "P4Merge" => file == "p4merge.exe",
        "WinMerge" => file == "winmergeu.exe" || file == "winmerge.exe",
        _ => false,
    }
}

fn builtin_args(kind: Kind, kind_type: &str, path: &str) -> Option<String> {
    if !builtin_matches(kind_type, path) {
        return None;
    }
    let args = match kind_type {
        "BeyondCompare" | "BeyondCompare3" | "BeyondCompare4" | "BeyondCompare5" => match kind {
            Kind::Merge => "{ours} {theirs} {base} {working}",
            Kind::Diff => "{left} {right}",
        },
        "AraxisMerge" | "Araxis" => match kind {
            Kind::Merge => "-wait -merge -3 -a1 {ours} {base} {theirs} {working}",
            Kind::Diff => "{left} {right}",
        },
        "KDiff3" => match kind {
            Kind::Merge => "{base} {ours} {theirs} -o {working}",
            Kind::Diff => "{left} {right}",
        },
        "Meld" => match kind {
            Kind::Merge => "{ours} {base} {theirs} -o {working}",
            Kind::Diff => "{left} {right}",
        },
        "P4Merge" => match kind {
            Kind::Merge => "{base} {ours} {theirs} {working}",
            Kind::Diff => "{left} {right}",
        },
        "WinMerge" => match kind {
            Kind::Merge => "-e -ub {ours} {theirs} {working}",
            Kind::Diff => "{left} {right}",
        },
        _ => return None,
    };
    Some(args.to_string())
}

/// Fork uses `$LOCAL` for the old side of a diff and for ours in a merge.
/// Compare arguments are `"$REMOTE" "$LOCAL"`, so the new side stays on the right.
fn translate(kind: Kind, raw: &str) -> String {
    let pairs: &[(&str, &str)] = match kind {
        Kind::Merge => &[("$MERGED", "{working}"), ("$LOCAL", "{ours}"), ("$BASE", "{base}"), ("$REMOTE", "{theirs}")],
        Kind::Diff => &[("$MERGED", "{file}"), ("$LOCAL", "{left}"), ("$BASE", "{base}"), ("$REMOTE", "{right}")],
    };
    let mut text = raw.to_string();
    for (from, to) in pairs {
        text = text.replace(from, to);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{pick_tool, tools_from_fork_json};

    const FORK: &str = r#"{
      "ExternalMergeTools": [
        {
          "Type": "Custom",
          "Name": "ExcelMergeFork",
          "Path": "D:\\MyGit\\ForkExcelMergeTool\\ExcelMergeFork.exe",
          "Arguments": "$LOCAL,$BASE,$REMOTE,$MERGED",
          "IsPrimary": true
        }
      ],
      "MergeTool": {
        "Type": "Custom",
        "ApplicationPath": "D:\\MyGit\\ForkExcelMergeTool\\ExcelMergeFork.exe",
        "Arguments": "$LOCAL,$BASE,$REMOTE,$MERGED"
      },
      "ExternalDiffTools": [
        { "Type": "BeyondCompare", "Path": "D:\\MyExe\\ExcelMerge\\ExcelMergeFork.exe" },
        {
          "Type": "Custom",
          "Name": "ExcelMergeFork",
          "Path": "D:\\MyGit\\ForkExcelMergeTool\\ExcelMergeFork.exe",
          "Arguments": "\"$REMOTE\" \"$LOCAL\""
        }
      ],
      "ExternalDiffTool": {
        "Type": "Custom",
        "ApplicationPath": "D:\\MyGit\\ForkExcelMergeTool\\ExcelMergeFork.exe",
        "Arguments": "\"$REMOTE\" \"$LOCAL\""
      }
    }"#;

    #[test]
    fn official_fork_tools_drop_a_mislabeled_compare_entry() {
        let tools = tools_from_fork_json(FORK, |path| path.ends_with("ExcelMergeFork.exe"));
        assert_eq!(tools.merge.len(), 1);
        assert_eq!(tools.merge[0].name, "ExcelMergeFork");
        assert_eq!(tools.merge[0].args, "{ours},{base},{theirs},{working}");
        assert!(tools.merge[0].primary);
        assert!(tools.merge[0].path.contains("ForkExcelMergeTool"));
        assert_eq!(tools.diff.len(), 1);
        assert_eq!(tools.diff[0].name, "ExcelMergeFork");
        assert_eq!(tools.diff[0].args, "\"{right}\" \"{left}\"");
        assert!(tools.diff[0].primary);
        assert!(tools.diff[0].path.contains("ForkExcelMergeTool"));
    }

    #[test]
    fn missing_programs_are_removed() {
        let tools = tools_from_fork_json(FORK, |_| false);
        assert!(tools.merge.is_empty());
        assert!(tools.diff.is_empty());
    }

    #[test]
    fn beyond_compare_stays_only_when_the_executable_matches() {
        let json = r#"{ "ExternalDiffTools": [ { "Type": "BeyondCompare", "Path": "C:\\Program Files\\Beyond Compare 4\\BComp.exe" } ] }"#;
        let tools = tools_from_fork_json(json, |_| true);
        assert_eq!(tools.diff.len(), 1);
        assert_eq!(tools.diff[0].name, "Beyond Compare");
        assert_eq!(tools.diff[0].args, "{left} {right}");
    }

    #[test]
    fn extension_tools_win_over_the_all_files_tool() {
        use crate::settings::ToolRecord;
        let tool = |id: &str, extensions: &[&str]| ToolRecord {
            id: id.into(),
            name: id.into(),
            path: format!("{id}.exe"),
            args: String::new(),
            extensions: extensions.iter().map(|item| (*item).to_string()).collect(),
            fallback: extensions.is_empty(),
        };
        let list = vec![tool("generic", &[]), tool("sheet", &["xlsx"]), tool("sheet2", &["*.XLSX"])];
        assert_eq!(pick_tool(&list, "data/book.xlsx").unwrap().id, "sheet");
        assert_eq!(pick_tool(&list, "src/main.rs").unwrap().id, "generic");
        assert_eq!(pick_tool(&list, "README").unwrap().id, "generic");
        assert_eq!(pick_tool(&[tool("only", &["txt"])], "a.md").unwrap().id, "only");
        let archives = vec![tool("generic", &[]), tool("archive", &["tar.gz"])];
        assert_eq!(pick_tool(&archives, "bundle.tar.gz").unwrap().id, "archive");
    }
}
