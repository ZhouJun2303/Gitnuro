# Build a Windows installer with pinned MinGit beside the app.
# Official Git for Windows only. This does not copy another app's Git.

$ErrorActionPreference = "Stop"
$version = "2.56.0"
$tag = "v2.56.0.windows.1"
$name = "MinGit-$version-64-bit.zip"
$sha256 = "064b440ff870ed5198527e8f3a92cdf5bd2fd0fedf5e718af95e3fdaddeff718"
$url = "https://github.com/git-for-windows/git/releases/download/$tag/$name"

$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$dest = Join-Path $root "src-tauri\resources\git"
$zip = Join-Path $env:TEMP $name

if (-not (Test-Path (Join-Path $dest "cmd\git.exe"))) {
    Write-Host "Downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $zip
    $actual = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower()
    if ($actual -ne $sha256) {
        throw "MinGit hash mismatch: $actual"
    }
    if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    Expand-Archive -Path $zip -DestinationPath $dest
    Set-Content -Path (Join-Path $dest ".gitkeep") -Value "MinGit is downloaded by scripts/package-windows.ps1 and is not committed." -Encoding ascii
}

if (-not (Test-Path (Join-Path $dest "cmd\git.exe"))) {
    throw "MinGit did not contain cmd\git.exe"
}

$mingw = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$env:PATH = "$mingw;$env:USERPROFILE\.cargo\bin;" + $env:PATH
Set-Location $root
pnpm tauri build
Write-Host "Installer output is under target\release\bundle"
