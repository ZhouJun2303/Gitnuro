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

# MinGit's etc/gitconfig includes Git for Windows' system config. That config
# can name `manager-core`, which MinGit does not ship, so every network command
# prints "not a git command" before another helper succeeds. Reset inherited
# helpers and keep the credential manager bundled with MinGit.
$gitconfig = Join-Path $dest "etc\gitconfig"
if (Test-Path -LiteralPath $gitconfig) {
    $text = Get-Content -LiteralPath $gitconfig -Raw
    if ($text -notmatch [regex]::Escape("AweGit: reset inherited credential helpers")) {
        $addition = @"

# AweGit: reset inherited credential helpers. Git for Windows may name
# manager-core, which MinGit does not ship; use the bundled credential manager.
[credential]
	helper =
	helper = manager
"@
        Add-Content -LiteralPath $gitconfig -Value $addition -Encoding ascii
    }
}

function Find-GccBin {
    if ($env:AWE_GCC) {
        if (Test-Path (Join-Path $env:AWE_GCC "gcc.exe")) { return $env:AWE_GCC }
        if (Test-Path $env:AWE_GCC -PathType Leaf) { return (Split-Path -Parent $env:AWE_GCC) }
    }
    $packages = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages"
    if (Test-Path $packages) {
        $hits = Get-ChildItem -Path $packages -Directory -Filter "BrechtSanders.WinLibs*" -ErrorAction SilentlyContinue
        foreach ($hit in $hits) {
            $bin = Join-Path $hit.FullName "mingw64\bin"
            if (Test-Path (Join-Path $bin "gcc.exe")) { return $bin }
        }
    }
    foreach ($root in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
        if (-not $root) { continue }
        $bin = Join-Path $root "WinLibs\mingw64\bin"
        if (Test-Path (Join-Path $bin "gcc.exe")) { return $bin }
    }
    foreach ($entry in ($env:PATH -split ';')) {
        $bin = $entry.Trim().Trim('"')
        if ($bin -and (Test-Path -LiteralPath (Join-Path $bin "gcc.exe") -PathType Leaf)) { return $bin }
    }
    return $null
}

$gccBin = Find-GccBin
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }
$cargoBin = Join-Path $cargoHome "bin"
if (-not (Test-Path (Join-Path $cargoBin "cargo.exe"))) {
    throw "未找到 cargo.exe。已查找 $(Join-Path $cargoBin 'cargo.exe')。可设置 CARGO_HOME 或 AWE_RUSTUP。"
}
if ($gccBin) { $env:PATH = "$gccBin;$cargoBin;" + $env:PATH } else { $env:PATH = "$cargoBin;" + $env:PATH }
Write-Host "cargo: $(Join-Path $cargoBin 'cargo.exe')"
$rustc = Join-Path $cargoBin "rustc.exe"
if (Test-Path $rustc) { Write-Host "rustc: $rustc" }
if ($gccBin) { Write-Host "gcc: $(Join-Path $gccBin 'gcc.exe')" } else { Write-Host "gcc: (未加入 PATH)" }
# Tauri embeds this bootstrapper for webviewInstallMode embedBootstrapper.
# Its own downloader often stops halfway on the Microsoft redirect and reports
# "io: unexpected end of file". A complete file in the cache is reused as-is.
function Test-WebView2Bootstrapper([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { return $false }
    $item = Get-Item -LiteralPath $path
    if ($item.Length -lt 1MB) { return $false }
    $stream = [IO.File]::OpenRead($path)
    try {
        $header = New-Object byte[] 2
        $read = $stream.Read($header, 0, 2)
        return $read -eq 2 -and $header[0] -eq 0x4D -and $header[1] -eq 0x5A
    } finally {
        $stream.Close()
    }
}

function Ensure-WebView2Bootstrapper {
    $cacheDir = Join-Path $env:LOCALAPPDATA "tauri"
    $dest = Join-Path $cacheDir "MicrosoftEdgeWebview2Setup.exe"
    if (Test-WebView2Bootstrapper $dest) {
        Write-Host "WebView2 bootstrapper cache: $dest"
        return
    }
    if (Test-Path -LiteralPath $dest) {
        Write-Host "WebView2 bootstrapper cache is incomplete. Downloading it again."
        Remove-Item -LiteralPath $dest -Force
    }
    if (-not (Get-Command curl.exe -ErrorAction SilentlyContinue)) {
        throw "没找到 curl.exe，无法下载 WebView2 引导程序。"
    }
    New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null
    $partial = "$dest.partial"
    $url = "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
    $ready = $false
    for ($try = 1; $try -le 4; $try++) {
        if (Test-Path -LiteralPath $partial) { Remove-Item -LiteralPath $partial -Force }
        Write-Host "Downloading WebView2 bootstrapper (try $try): $url"
        & curl.exe -L --fail --silent --show-error --retry 3 --retry-all-errors --retry-delay 2 --connect-timeout 20 --max-time 180 -o $partial $url
        if ($LASTEXITCODE -eq 0 -and (Test-WebView2Bootstrapper $partial)) {
            Move-Item -LiteralPath $partial -Destination $dest -Force
            $ready = $true
            break
        }
        Start-Sleep -Seconds (2 * $try)
    }
    if (Test-Path -LiteralPath $partial) { Remove-Item -LiteralPath $partial -Force }
    if (-not $ready) {
        throw "没能下载完整的 WebView2 引导程序。安装包需要把它打进去，请检查网络后重试。"
    }
    Write-Host "WebView2 bootstrapper cache: $dest"
}

Ensure-WebView2Bootstrapper
Set-Location $root
pnpm tauri build
Write-Host "Installer output is under target\release\bundle"
