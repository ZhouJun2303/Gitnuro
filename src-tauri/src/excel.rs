//! Windows Excel coordination for Git operations that replace work-tree files.
//!
//! Excel exposes its running workbooks through COM.  Keeping the automation in a
//! small PowerShell process avoids adding a Windows-only COM dependency to the
//! cross-platform shell while still allowing the desktop build to save, close,
//! and reopen workbooks around a pull.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Session {
    paths: Vec<PathBuf>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrepareResult {
    #[serde(default)]
    paths: Vec<String>,
}

/// Save and close open Excel workbooks that belong to `repo`.
///
/// Non-Windows builds deliberately do nothing.  The setting remains portable,
/// but Excel COM automation is only available in the Windows desktop build.
pub fn prepare(repo: &Path) -> Result<Option<Session>, String> {
    #[cfg(windows)]
    {
        let output = run_powershell(
            PREPARE_SCRIPT,
            Some(("AWEGIT_EXCEL_REPO", repo.display().to_string())),
        )?;
        if !output.status.success() {
            return Err(process_error(&output));
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let result: PrepareResult = serde_json::from_str(&text)
            .map_err(|error| format!("Excel detection returned invalid data: {error}"))?;
        let paths = result
            .paths
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        return Ok((!paths.is_empty()).then_some(Session { paths }));
    }
    #[cfg(not(windows))]
    {
        let _ = repo;
        Ok(None)
    }
}

/// Reopen workbooks saved by [`prepare`].  Missing files are ignored because a
/// successful pull may have deleted or renamed a workbook.
pub fn restore(session: &Session) -> Result<(), String> {
    #[cfg(windows)]
    {
        let paths = session
            .paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        let payload = serde_json::to_string(&paths).map_err(|error| error.to_string())?;
        let output = run_powershell(RESTORE_SCRIPT, Some(("AWEGIT_EXCEL_PATHS", payload)))?;
        if !output.status.success() {
            return Err(process_error(&output));
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = session;
        Ok(())
    }
}

#[cfg(windows)]
fn run_powershell(
    script: &str,
    environment: Option<(&str, String)>,
) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = std::process::Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW);
    if let Some((name, value)) = environment {
        command.env(name, value);
    }
    command.output().map_err(|error| error.to_string())
}

#[cfg(windows)]
fn process_error(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            "Excel automation failed.".into()
        } else {
            stdout
        }
    } else {
        stderr
    }
}

#[cfg(windows)]
const PREPARE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function Normalize-ExcelPath([string] $value) {
  if ([string]::IsNullOrWhiteSpace($value)) { return '' }
  try { return ([IO.Path]::GetFullPath($value)).TrimEnd('\').ToLowerInvariant() } catch { return '' }
}
$repo = Normalize-ExcelPath $env:AWEGIT_EXCEL_REPO
try {
  $excel = [Runtime.InteropServices.Marshal]::GetActiveObject('Excel.Application')
} catch {
  [pscustomobject]@{ paths = [string[]]@() } | ConvertTo-Json -Compress
  exit 0
}
$books = @($excel.Workbooks | Where-Object {
  $path = Normalize-ExcelPath ([string]$_.FullName)
  $path -eq $repo -or $path.StartsWith($repo + '\')
})
$paths = New-Object 'System.Collections.Generic.List[string]'
$alerts = $excel.DisplayAlerts
try {
  $excel.DisplayAlerts = $false
  foreach ($book in $books) {
    if (-not $book.Saved) { $book.Save() }
  }
  foreach ($book in $books) {
    $path = [string]$book.FullName
    $book.Close($false)
    [void]$paths.Add($path)
  }
} finally {
  try { $excel.DisplayAlerts = $alerts } catch { }
}
if ($books.Count -gt 0 -and @($excel.Workbooks).Count -eq 0) {
  $excel.Quit()
}
[pscustomobject]@{ paths = [string[]]$paths } | ConvertTo-Json -Compress
"#;

#[cfg(windows)]
const RESTORE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function Normalize-ExcelPath([string] $value) {
  if ([string]::IsNullOrWhiteSpace($value)) { return '' }
  try { return ([IO.Path]::GetFullPath($value)).TrimEnd('\').ToLowerInvariant() } catch { return '' }
}
$paths = @((ConvertFrom-Json $env:AWEGIT_EXCEL_PATHS))
if ($paths.Count -eq 0) { exit 0 }
$existing = @($paths | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
if ($existing.Count -eq 0) { exit 0 }
try {
  $excel = [Runtime.InteropServices.Marshal]::GetActiveObject('Excel.Application')
} catch {
  $excel = New-Object -ComObject Excel.Application
}
$excel.Visible = $true
$open = @($excel.Workbooks | ForEach-Object { Normalize-ExcelPath ([string]$_.FullName) })
foreach ($path in $existing) {
  if ($open -contains (Normalize-ExcelPath ([string]$path))) { continue }
  [void]$excel.Workbooks.Open([string]$path)
}
$excel.UserControl = $true
"#;
