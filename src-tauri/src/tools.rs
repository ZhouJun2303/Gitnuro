//! Installed external tools and imports from Fork's settings.
//!
//! A built-in Fork type is kept only when the file on disk is that program.
//! A path that is missing, or a Beyond Compare entry that points at another executable, is dropped.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
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

/// An installed tool with arguments for each supported mode (empty means unsupported).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonTool {
    pub name: String,
    pub path: String,
    pub diff_args: String,
    pub merge_args: String,
}

/// A text editor present on this machine, used to seed the editor list.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorTool {
    pub name: String,
    pub path: String,
    pub args: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalCli {
    Codex,
    Grok,
    CursorAgent,
    Claude,
}

impl TerminalCli {
    const ALL: [Self; 4] = [Self::Codex, Self::Grok, Self::CursorAgent, Self::Claude];

    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Grok => "grok",
            Self::CursorAgent => "cursor-agent",
            Self::Claude => "claude",
        }
    }

    fn resolve(self, paths: &[PathBuf]) -> Option<PathBuf> {
        #[cfg(windows)]
        let extensions = ["exe", "cmd", "bat"];
        #[cfg(not(windows))]
        let extensions = [""];
        paths.iter().filter(|dir| dir.is_absolute()).find_map(|dir| {
            extensions.iter().find_map(|extension| {
                let path = dir.join(self.name()).with_extension(extension);
                executable_file(&path).then_some(path)
            })
        })
    }
}

/// Resolve a detected terminal CLI to its executable or Windows shim.
pub fn resolve_terminal_cli(cli: TerminalCli) -> Option<PathBuf> {
    cli.resolve(&terminal_cli_paths())
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else { return false };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    metadata.is_file()
}

fn terminal_cli_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            paths.push(PathBuf::from(appdata).join("npm"));
        }
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            paths.push(PathBuf::from(profile).join(".local").join("bin"));
        }
    }
    #[cfg(unix)]
    {
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(PathBuf::from(home).join(".local/bin"));
        }
        paths.push(PathBuf::from("/usr/local/bin"));
    }
    #[cfg(target_os = "macos")]
    paths.push(PathBuf::from("/opt/homebrew/bin"));
    paths
}

pub fn detected_terminal_clis() -> Vec<TerminalCli> {
    let paths = terminal_cli_paths();
    TerminalCli::ALL.into_iter().filter(|cli| cli.resolve(&paths).is_some()).collect()
}

#[cfg(windows)]
fn windows_terminal_cli(tool: &Path, repo: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    let system_dir = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "SystemRoot is missing"))?
        .join("System32");
    let shell = system_dir.join("cmd.exe");
    let mut command = Command::new(&shell);
    // START allocates the console. The inner cmd expands the project and tool paths once.
    command.args(["/D", "/E:ON", "/V:OFF", "/C"])
        .raw_arg(r#"start "" "%AWEGIT_TERMINAL_SHELL%" /D /E:ON /V:OFF /S /K "%AWEGIT_TERMINAL_COMMAND%""#)
        .env("AWEGIT_TERMINAL_SHELL", &shell)
        .env("AWEGIT_TERMINAL_COMMAND", r#"pushd "%AWEGIT_TERMINAL_PROJECT%" && "%AWEGIT_TERMINAL_CLI%""#)
        .env("AWEGIT_TERMINAL_PROJECT", repo)
        .env("AWEGIT_TERMINAL_CLI", tool)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(system_dir);
    awegit_git::hide_console(&mut command);
    let status = command.status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!("cmd.exe {status}")));
    }
    Ok(())
}

pub fn open_terminal_cli(cli: TerminalCli, repo: &Path) -> Result<(), String> {
    let tool = cli.resolve(&terminal_cli_paths()).ok_or_else(|| "terminal-cli-missing".to_string())?;
    #[cfg(windows)]
    windows_terminal_cli(&tool, repo).map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/usr/bin/osascript")
            .args([
                "-e", "on run argv",
                "-e", "tell application \"Terminal\"",
                "-e", "do script (\"cd -- \" & quoted form of item 1 of argv & \" && exec \" & quoted form of item 2 of argv)",
                "-e", "activate",
                "-e", "end tell",
                "-e", "end run",
            ])
            .arg(repo).arg(tool).output().map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    Command::new("x-terminal-emulator").arg("-e").arg(tool).current_dir(repo)
        .spawn().map_err(|error| error.to_string())?;
    Ok(())
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

/// Tools from install folders, PATH and Windows App Paths. No programs are launched.
pub fn common_tools() -> Vec<CommonTool> {
    installed_common_tools(common_presets(), program_exists)
}

fn installed_common_tools(presets: Vec<CommonTool>, exists: impl Fn(&str) -> bool) -> Vec<CommonTool> {
    let mut tools: Vec<CommonTool> = Vec::new();
    for preset in presets {
        if preset.path.trim().is_empty() || !exists(&preset.path) {
            continue;
        }
        if tools.iter().any(|item| item.name.eq_ignore_ascii_case(&preset.name) || same_path(&item.path, &preset.path)) {
            continue;
        }
        tools.push(preset);
    }
    tools
}

fn common_presets() -> Vec<CommonTool> {
    common_presets_with(&registered_programs())
}

fn common_presets_with(registered: &[(String, String)]) -> Vec<CommonTool> {
    let mut rows = Vec::new();
    for preset in COMMON_PRESETS {
        let mut paths = Vec::new();
        for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            let Some(root) = std::env::var_os(variable).map(std::path::PathBuf::from) else { continue };
            for folder in preset.folders {
                for file in preset.executables.iter().filter(|file| file.ends_with(".exe")) {
                    paths.push(root.join(folder).join(file).display().to_string());
                    if variable == "LOCALAPPDATA" {
                        paths.push(root.join("Programs").join(folder).join(file).display().to_string());
                    }
                }
            }
        }
        paths.extend(registered.iter()
            .filter(|(file, _)| preset.executables.iter().any(|exe| file.eq_ignore_ascii_case(exe)))
            .map(|(_, path)| path.clone()));
        if cfg!(target_os = "macos") {
            for app_path in preset.mac_paths {
                paths.push(format!("/Applications/{app_path}"));
                if let Some(home) = std::env::var_os("HOME") {
                    paths.push(Path::new(&home).join("Applications").join(app_path).display().to_string());
                }
            }
            if preset.name == "FileMerge" {
                paths.push("/Library/Developer/CommandLineTools/usr/bin/opendiff".into());
                paths.push("/Applications/Xcode.app/Contents/Developer/usr/bin/opendiff".into());
            }
        }
        paths.extend(preset.executables.iter().map(|file| on_path(file)).filter(|path| !path.is_empty()));
        // `compare.exe` is also shipped by unrelated programs such as ImageMagick.
        rows.extend(paths.into_iter()
            .filter(|path| preset.name != "Araxis Merge" || path.to_ascii_lowercase().contains("araxis"))
            .map(|path| CommonTool {
            name: preset.name.into(), path,
            diff_args: preset.diff_args.into(), merge_args: preset.merge_args.into(),
        }));
    }
    rows
}

struct CommonPreset {
    name: &'static str,
    folders: &'static [&'static str],
    executables: &'static [&'static str],
    mac_paths: &'static [&'static str],
    diff_args: &'static str,
    merge_args: &'static str,
}

const DIFF_ARGS: &str = "\"{left}\" \"{right}\"";
const CODE_DIFF_ARGS: &str = "--wait --diff \"{left}\" \"{right}\"";
const CODE_MERGE_ARGS: &str = "--wait --merge \"{ours}\" \"{theirs}\" \"{base}\" \"{working}\"";
const TORTOISE_DIFF_ARGS: &str = "-base:\"{left}\" -mine:\"{right}\"";
const TORTOISE_MERGE_ARGS: &str = "-base:\"{base}\" -mine:\"{ours}\" -theirs:\"{theirs}\" -merged:\"{working}\"";

const COMMON_PRESETS: &[CommonPreset] = &[
    CommonPreset {
        name: "Beyond Compare", folders: &["Beyond Compare 5", "Beyond Compare 4", "Beyond Compare 3"],
        executables: &["BComp.exe", "BCompare.exe", "bcomp", "bcompare"],
        mac_paths: &["Beyond Compare.app/Contents/MacOS/bcomp"],
        diff_args: DIFF_ARGS, merge_args: "\"{ours}\" \"{theirs}\" \"{base}\" \"{working}\"",
    },
    CommonPreset {
        name: "WinMerge", folders: &["WinMerge"], executables: &["WinMergeU.exe", "WinMerge.exe"], mac_paths: &[],
        diff_args: DIFF_ARGS, merge_args: "-e -u -o \"{working}\" \"{ours}\" \"{base}\" \"{theirs}\"",
    },
    CommonPreset {
        name: "Araxis Merge", folders: &["Araxis\\Araxis Merge", "Araxis Merge"],
        executables: &["Compare.exe", "Merge.exe", "araxis"], mac_paths: &["Araxis Merge.app/Contents/Utilities/compare"],
        diff_args: "-wait \"{left}\" \"{right}\"", merge_args: "-wait -merge -3 -a1 \"{ours}\" \"{base}\" \"{theirs}\" \"{working}\"",
    },
    CommonPreset {
        name: "KDiff3", folders: &["KDiff3"], executables: &["kdiff3.exe", "kdiff3"],
        mac_paths: &["kdiff3.app/Contents/MacOS/kdiff3", "KDiff3.app/Contents/MacOS/kdiff3"],
        diff_args: DIFF_ARGS, merge_args: "\"{base}\" \"{ours}\" \"{theirs}\" -o \"{working}\"",
    },
    CommonPreset {
        name: "Meld", folders: &["Meld"], executables: &["Meld.exe", "meld"], mac_paths: &["Meld.app/Contents/MacOS/Meld"],
        diff_args: DIFF_ARGS, merge_args: "\"{ours}\" \"{base}\" \"{theirs}\" -o \"{working}\"",
    },
    CommonPreset {
        name: "P4Merge", folders: &["Perforce", "Perforce\\P4Merge"], executables: &["p4merge.exe", "p4merge"],
        mac_paths: &["p4merge.app/Contents/MacOS/p4merge"],
        diff_args: DIFF_ARGS, merge_args: "\"{base}\" \"{ours}\" \"{theirs}\" \"{working}\"",
    },
    CommonPreset {
        name: "DiffMerge", folders: &["SourceGear\\Common\\DiffMerge", "SourceGear\\DiffMerge", "DiffMerge"],
        executables: &["sgdm.exe", "DiffMerge.exe", "diffmerge"], mac_paths: &["DiffMerge.app/Contents/MacOS/DiffMerge"],
        diff_args: DIFF_ARGS, merge_args: "--merge --result=\"{working}\" \"{ours}\" \"{base}\" \"{theirs}\"",
    },
    CommonPreset {
        name: "TortoiseGitMerge", folders: &["TortoiseGit\\bin"], executables: &["TortoiseGitMerge.exe"], mac_paths: &[],
        diff_args: TORTOISE_DIFF_ARGS, merge_args: TORTOISE_MERGE_ARGS,
    },
    CommonPreset {
        name: "TortoiseMerge", folders: &["TortoiseSVN\\bin"], executables: &["TortoiseMerge.exe"], mac_paths: &[],
        diff_args: TORTOISE_DIFF_ARGS, merge_args: TORTOISE_MERGE_ARGS,
    },
    CommonPreset {
        name: "Code Compare", folders: &["Devart\\Code Compare"], executables: &["CodeCompare.exe"], mac_paths: &[],
        diff_args: DIFF_ARGS, merge_args: "",
    },
    CommonPreset {
        name: "Code Merge", folders: &["Devart\\Code Compare"], executables: &["CodeMerge.exe"], mac_paths: &[],
        diff_args: "", merge_args: "-MF=\"{ours}\" -BF=\"{base}\" -TF=\"{theirs}\" -RF=\"{working}\"",
    },
    CommonPreset {
        name: "VS Code", folders: &["Microsoft VS Code"], executables: &["Code.exe", "code.cmd", "code"],
        mac_paths: &["Visual Studio Code.app/Contents/Resources/app/bin/code"],
        diff_args: CODE_DIFF_ARGS, merge_args: CODE_MERGE_ARGS,
    },
    CommonPreset {
        name: "VS Code Insiders", folders: &["Microsoft VS Code Insiders"], executables: &["Code - Insiders.exe", "code-insiders.cmd", "code-insiders"],
        mac_paths: &["Visual Studio Code - Insiders.app/Contents/Resources/app/bin/code-insiders"],
        diff_args: CODE_DIFF_ARGS, merge_args: CODE_MERGE_ARGS,
    },
    CommonPreset {
        name: "Cursor", folders: &["cursor"], executables: &["Cursor.exe", "cursor.cmd", "cursor"],
        mac_paths: &["Cursor.app/Contents/Resources/app/bin/cursor"], diff_args: CODE_DIFF_ARGS, merge_args: CODE_MERGE_ARGS,
    },
    CommonPreset {
        name: "Windsurf", folders: &["Windsurf"], executables: &["Windsurf.exe", "windsurf.cmd", "windsurf"],
        mac_paths: &["Windsurf.app/Contents/Resources/app/bin/windsurf"], diff_args: CODE_DIFF_ARGS, merge_args: CODE_MERGE_ARGS,
    },
    CommonPreset {
        name: "VSCodium", folders: &["VSCodium"], executables: &["VSCodium.exe", "codium.cmd", "codium"],
        mac_paths: &["VSCodium.app/Contents/Resources/app/bin/codium"], diff_args: CODE_DIFF_ARGS, merge_args: CODE_MERGE_ARGS,
    },
    CommonPreset {
        name: "FileMerge", folders: &[], executables: &["opendiff"], mac_paths: &[],
        diff_args: DIFF_ARGS, merge_args: "\"{ours}\" \"{theirs}\" -ancestor \"{base}\" -merge \"{working}\"",
    },
    CommonPreset {
        name: "Kaleidoscope", folders: &[], executables: &["ksdiff"], mac_paths: &["Kaleidoscope.app/Contents/MacOS/ksdiff"],
        diff_args: "--wait \"{left}\" \"{right}\"", merge_args: "--wait --merge --output \"{working}\" --base \"{base}\" \"{ours}\" \"{theirs}\"",
    },
];

#[cfg(windows)]
fn registered_programs() -> Vec<(String, String)> {
    use std::os::windows::process::CommandExt;
    // Registry APIs preserve Unicode paths and expand REG_EXPAND_SZ values.
    // Query both views without changing the registry or starting any detected tool.
    const SCRIPT: &str = r#"
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new()
        $found = [Collections.Generic.List[object]]::new()
        foreach ($hive in @('CurrentUser', 'LocalMachine')) {
            foreach ($view in @('Registry64', 'Registry32')) {
                $root = $null; $apps = $null
                try {
                    $root = [Microsoft.Win32.RegistryKey]::OpenBaseKey($hive, $view)
                    $apps = $root.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\App Paths')
                    if ($null -eq $apps) { continue }
                    foreach ($name in $apps.GetSubKeyNames()) {
                        if (!$name.EndsWith('.exe', [StringComparison]::OrdinalIgnoreCase)) { continue }
                        $app = $null
                        try {
                            $app = $apps.OpenSubKey($name)
                            if ($null -eq $app) { continue }
                            $path = $app.GetValue('')
                            if ($path -is [string] -and $path) { $found.Add(@($name, $path.Trim('"'))) }
                        } catch {} finally { if ($null -ne $app) { $app.Dispose() } }
                    }
                } catch {} finally {
                    if ($null -ne $apps) { $apps.Dispose() }
                    if ($null -ne $root) { $root.Dispose() }
                }
            }
        }
        ConvertTo-Json -InputObject @($found.ToArray()) -Depth 3 -Compress
    "#;
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .creation_flags(0x08000000).output().ok()
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice(&output.stdout).ok())
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn registered_programs() -> Vec<(String, String)> { Vec::new() }

/// Editors installed on this machine, newest names first. Only programs present on disk are returned.
pub fn editor_tools() -> Vec<EditorTool> {
    let mut tools: Vec<EditorTool> = Vec::new();
    for preset in editor_presets() {
        if preset.path.trim().is_empty() || !program_exists(&preset.path) {
            continue;
        }
        if tools.iter().any(|item| item.name.eq_ignore_ascii_case(&preset.name)) {
            continue;
        }
        tools.push(preset);
    }
    tools
}

fn editor_presets() -> Vec<EditorTool> {
    let local = |folder: &str, file: &str| {
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|base| Path::new(&base).join(folder).join(file).display().to_string())
            .unwrap_or_default()
    };
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
    let goto = "-g \"{file}:{line}\"";
    let mut rows = vec![
        EditorTool {
            name: "Visual Studio Code".into(),
            path: local("Programs\\Microsoft VS Code", "Code.exe"),
            args: goto.into(),
        },
        EditorTool {
            name: "Visual Studio Code".into(),
            path: program("Microsoft VS Code", "Code.exe"),
            args: goto.into(),
        },
        EditorTool {
            name: "Visual Studio Code".into(),
            path: on_path("code.cmd"),
            args: goto.into(),
        },
        EditorTool {
            name: "Visual Studio Code".into(),
            path: on_path("code.exe"),
            args: goto.into(),
        },
        EditorTool {
            name: "VS Code Insiders".into(),
            path: local("Programs\\Microsoft VS Code Insiders", "Code - Insiders.exe"),
            args: goto.into(),
        },
        EditorTool {
            name: "Cursor".into(),
            path: local("Programs\\cursor", "Cursor.exe"),
            args: goto.into(),
        },
        EditorTool {
            name: "Cursor".into(),
            path: local("Programs\\Cursor", "Cursor.exe"),
            args: goto.into(),
        },
        EditorTool {
            name: "Cursor".into(),
            path: local("Programs\\cursor\\resources\\app\\bin", "cursor.cmd"),
            args: goto.into(),
        },
        EditorTool {
            name: "Cursor".into(),
            path: program("Cursor", "Cursor.exe"),
            args: goto.into(),
        },
        EditorTool {
            name: "Cursor".into(),
            path: on_path("cursor.exe"),
            args: goto.into(),
        },
        EditorTool {
            name: "Cursor".into(),
            path: on_path("cursor.cmd"),
            args: goto.into(),
        },
        EditorTool {
            name: "Windsurf".into(),
            path: local("Programs\\Windsurf", "Windsurf.exe"),
            args: goto.into(),
        },
        EditorTool {
            name: "Windsurf".into(),
            path: program("Windsurf", "Windsurf.exe"),
            args: goto.into(),
        },
        EditorTool {
            name: "Windsurf".into(),
            path: on_path("windsurf.cmd"),
            args: goto.into(),
        },
        EditorTool {
            name: "Sublime Text".into(),
            path: program("Sublime Text", "sublime_text.exe"),
            args: "\"{file}:{line}\"".into(),
        },
        EditorTool {
            name: "Sublime Text".into(),
            path: program_x86("Sublime Text", "sublime_text.exe"),
            args: "\"{file}:{line}\"".into(),
        },
        EditorTool {
            name: "Notepad++".into(),
            path: program("Notepad++", "notepad++.exe"),
            args: "-n{line} \"{file}\"".into(),
        },
        EditorTool {
            name: "Notepad++".into(),
            path: program_x86("Notepad++", "notepad++.exe"),
            args: "-n{line} \"{file}\"".into(),
        },
        EditorTool {
            name: "Zed".into(),
            path: local("Programs\\Zed", "Zed.exe"),
            args: "\"{file}:{line}\"".into(),
        },
        EditorTool {
            name: "Zed".into(),
            path: program("Zed", "Zed.exe"),
            args: "\"{file}:{line}\"".into(),
        },
        EditorTool {
            name: "Zed".into(),
            path: on_path("zed.exe"),
            args: "\"{file}:{line}\"".into(),
        },
    ];
    rows.extend(rider_paths().into_iter().map(|path| EditorTool {
        name: "Rider".into(),
        path,
        args: "--line {line} \"{file}\"".into(),
    }));
    rows
}

/// Rider lives under versioned folders, so its launcher is found by scanning.
/// Toolbox-managed installs come first, then the standalone installer and PATH.
fn rider_paths() -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    if let Some(base) = std::env::var("LOCALAPPDATA").ok().map(std::path::PathBuf::from) {
        let toolbox = base.join("JetBrains").join("Toolbox");
        found.extend(installed_under(&toolbox.join("apps").join("Rider"), "rider64.exe"));
        found.extend(existing_file(base.join("Programs").join("Rider").join("bin").join("rider64.exe")));
        found.extend(existing_file(toolbox.join("scripts").join("rider.cmd")));
    }
    for env in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var(env).ok().map(std::path::PathBuf::from) {
            found.extend(installed_under(&base.join("JetBrains"), "rider64.exe"));
        }
    }
    for launcher in ["rider64.exe", "rider.cmd"] {
        found.extend(existing_file(std::path::PathBuf::from(on_path(launcher))));
    }
    let mut unique: Vec<String> = Vec::new();
    for path in found {
        if !unique.iter().any(|item| same_path(item, &path)) {
            unique.push(path);
        }
    }
    unique
}

fn existing_file(path: std::path::PathBuf) -> Option<String> {
    path.is_file().then(|| path.display().to_string())
}

/// Launchers at `<root>\<folder>\bin\<file>` and, for Toolbox, one folder
/// deeper at `<root>\<channel>\<version>\bin\<file>`. Newest folder first.
fn installed_under(root: &Path, file: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut hits: Vec<(Vec<u32>, String)> = Vec::new();
    for entry in entries.flatten() {
        let folder = entry.path();
        if !folder.is_dir() {
            continue;
        }
        let direct = folder.join("bin").join(file);
        if direct.is_file() {
            hits.push((version_key(&folder.file_name().unwrap_or_default().to_string_lossy()), direct.display().to_string()));
            continue;
        }
        let Ok(versions) = std::fs::read_dir(&folder) else {
            continue;
        };
        for version in versions.flatten() {
            let nested = version.path().join("bin").join(file);
            if version.path().is_dir() && nested.is_file() {
                hits.push((version_key(&version.file_name().to_string_lossy()), nested.display().to_string()));
            }
        }
    }
    hits.sort_by(|left, right| right.0.cmp(&left.0));
    hits.into_iter().map(|(_, path)| path).collect()
}

/// `2024.3` outranks `2023.2`; a channel folder such as `ch-0` keeps its number.
fn version_key(name: &str) -> Vec<u32> {
    name.split(|value: char| !value.is_ascii_digit())
        .filter_map(|part| part.parse::<u32>().ok())
        .collect()
}

fn on_path(file: &str) -> String {
    let Some(paths) = std::env::var_os("PATH") else {
        return String::new();
    };
    std::env::split_paths(&paths)
        .map(|folder| folder.join(file))
        .find(|candidate| candidate.is_file())
        .map(|candidate| candidate.display().to_string())
        .unwrap_or_default()
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

    #[test]
    fn terminal_cli_detection_accepts_launchers_and_skips_missing_or_removed_files() {
        use super::TerminalCli;
        use std::fs;

        let directory = std::env::temp_dir().join(format!("awegit-cli detection-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        #[cfg(windows)]
        let launcher = directory.join("codex.cmd");
        #[cfg(not(windows))]
        let launcher = directory.join("codex");
        fs::write(&launcher, "").unwrap();
        let paths = std::slice::from_ref(&directory);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&launcher, fs::Permissions::from_mode(0o644)).unwrap();
            assert_eq!(TerminalCli::Codex.resolve(paths), None);
            fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(TerminalCli::Codex.resolve(paths), Some(launcher.clone()));
        assert_eq!(TerminalCli::Grok.resolve(paths), None);
        assert_eq!(serde_json::to_string(&TerminalCli::CursorAgent).unwrap(), "\"cursor-agent\"");
        fs::remove_file(launcher).unwrap();
        assert_eq!(TerminalCli::Codex.resolve(paths), None);
        fs::remove_dir(directory).unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "opens a console and requires a Windows desktop session"]
    fn windows_terminal_cli_runs_batch_shims_in_the_project_directory() {
        use super::windows_terminal_cli;
        use std::fs;
        use std::time::{Duration, Instant};

        let directory = std::env::temp_dir().join(format!("awegit CLI launch-{}", std::process::id()));
        let repo = directory.join("project with spaces & %TEMP% !");
        let launcher = directory.join("grok %TEMP% & !.cmd");
        let marker = repo.join("cli-marker.txt");
        let released = repo.join("launcher-returned.txt");
        fs::create_dir_all(&repo).unwrap();
        fs::write(&launcher, concat!(
            "@echo off\r\n",
            "powershell.exe -NoProfile -NonInteractive -Command \"",
            "$deadline = [DateTime]::UtcNow.AddSeconds(5); ",
            "while (-not [IO.File]::Exists('launcher-returned.txt') -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 20 }; ",
            "$attached = -not ([Console]::IsInputRedirected -or [Console]::IsOutputRedirected -or [Console]::IsErrorRedirected); ",
            "[IO.File]::WriteAllText('cli-marker.txt', [string]($attached -and [IO.File]::Exists('launcher-returned.txt')))\"\r\n",
            "popd\r\n",
            "exit\r\n",
        )).unwrap();
        windows_terminal_cli(&launcher, &repo).unwrap();
        fs::write(&released, "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let contents = loop {
            if let Ok(text) = fs::read_to_string(&marker) {
                if !text.is_empty() {
                    break text;
                }
            }
            assert!(Instant::now() < deadline, "launcher did not write to {}", repo.display());
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(contents, "True");
        let _ = fs::remove_file(released);
        let _ = fs::remove_file(marker);
        let _ = fs::remove_file(launcher);
        let _ = fs::remove_dir(repo);
        let _ = fs::remove_dir(directory);
    }

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
    fn common_detection_skips_missing_programs_and_prefers_one_launcher_per_tool() {
        use super::{installed_common_tools, CommonTool};
        let preset = |name: &str, path: &str| CommonTool {
            name: name.into(), path: path.into(), diff_args: "{left} {right}".into(), merge_args: String::new(),
        };
        let tools = installed_common_tools(vec![
            preset("Beyond Compare", "missing.exe"),
            preset("Beyond Compare", "C:/Beyond Compare 5/BComp.exe"),
            preset("Beyond Compare", "C:/Beyond Compare 4/BCompare.exe"),
            preset("VS Code", "D:/Portable Code/Code.exe"),
            preset("VS Code", "D:/Portable Code/bin/code.cmd"),
            preset("Another alias", "d:\\portable code\\CODE.EXE"),
            preset("WinMerge", ""),
        ], |path| path != "missing.exe");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].path, "C:/Beyond Compare 5/BComp.exe");
        assert_eq!(tools[1].path, "D:/Portable Code/Code.exe");
    }

    #[test]
    fn common_detection_finds_portable_tools_on_path() {
        use super::common_presets_with;
        let root = std::env::temp_dir().join(format!("awegit-tool-detection-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let program = root.join("WinMergeU.exe");
        std::fs::write(&program, "").unwrap();
        let previous = std::env::var_os("PATH");
        std::env::set_var("PATH", &root);
        let found = common_presets_with(&[]);
        match previous {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
        let _ = std::fs::remove_file(&program);
        let _ = std::fs::remove_dir(root);
        assert!(found.iter().any(|tool| tool.name == "WinMerge" && std::path::Path::new(&tool.path) == program));
    }

    #[test]
    fn common_detection_accepts_registered_tools_in_unicode_custom_folders() {
        use super::{common_presets_with, installed_common_tools};
        let path = "D:\\我的工具\\Beyond Compare\\BComp.exe";
        let tools = installed_common_tools(common_presets_with(&[
            ("bcomp.EXE".into(), path.into()),
            ("Compare.exe".into(), "D:\\ImageMagick\\compare.exe".into()),
        ]), |candidate| candidate == path || candidate == "D:\\ImageMagick\\compare.exe");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "Beyond Compare");
        assert_eq!(tools[0].path, path);
    }

    #[test]
    fn common_tools_keep_three_way_merge_order_and_mode_capabilities() {
        use super::COMMON_PRESETS;
        let args = |name: &str| COMMON_PRESETS.iter().find(|preset| preset.name == name).unwrap();
        assert_eq!(args("WinMerge").merge_args, "-e -u -o \"{working}\" \"{ours}\" \"{base}\" \"{theirs}\"");
        assert_eq!(args("VS Code").merge_args, "--wait --merge \"{ours}\" \"{theirs}\" \"{base}\" \"{working}\"");
        assert_eq!(args("TortoiseGitMerge").diff_args, "-base:\"{left}\" -mine:\"{right}\"");
        assert!(args("Code Compare").merge_args.is_empty());
        assert!(args("Code Merge").diff_args.is_empty());
    }

    #[test]
    fn newer_install_folders_sort_first() {
        use super::version_key;
        let mut keys = vec![version_key("JetBrains Rider 2023.3"), version_key("2024.2.5"), version_key("ch-0")];
        keys.sort_by(|left, right| right.cmp(left));
        assert_eq!(keys[0], vec![2024, 2, 5]);
        assert_eq!(keys[1], vec![2023, 3]);
        assert_eq!(keys[2], vec![0]);
    }

    #[test]
    fn rider_launchers_are_found_in_both_install_layouts() {
        use super::installed_under;
        let root = std::env::temp_dir().join(format!("awegit-rider-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let direct = root.join("JetBrains Rider 2023.3").join("bin");
        let nested = root.join("ch-0").join("2024.2").join("bin");
        std::fs::create_dir_all(&direct).unwrap();
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(direct.join("rider64.exe"), "").unwrap();
        std::fs::write(nested.join("rider64.exe"), "").unwrap();
        let found = installed_under(&root, "rider64.exe");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(found.len(), 2);
        assert!(found[0].contains("2024.2"), "{found:?}");
        assert!(found[1].contains("2023.3"), "{found:?}");
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
