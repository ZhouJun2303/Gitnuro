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
Set-Location $root
pnpm tauri build
Write-Host "Installer output is under target\release\bundle"
