# Package AweGit for Windows, upload the installers, and publish the version manifest.
# Run this on the release branch. It commits latest.json at the repo root and pushes origin release.

param(
    [string]$Notes = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$repo = "ZhouJun2303/Gitnuro"

Push-Location $root
try {
    $current = (git rev-parse --abbrev-ref HEAD).Trim()
    if ($current -ne "release") {
        throw "Publish from the release branch. Current branch is $current"
    }
}
finally {
    Pop-Location
}

function Read-JsonFile([string]$path) {
    Get-Content -Raw -Encoding UTF8 -Path $path | ConvertFrom-Json
}

$tauriPath = Join-Path $root "src-tauri\tauri.conf.json"
$packagePath = Join-Path $root "package.json"
$cargoPath = Join-Path $root "src-tauri\Cargo.toml"
$gitCargoPath = Join-Path $root "crates\git\Cargo.toml"
$tauri = Read-JsonFile $tauriPath
$package = Read-JsonFile $packagePath
$cargoText = Get-Content -Raw -Encoding UTF8 -Path $cargoPath
$gitCargoText = Get-Content -Raw -Encoding UTF8 -Path $gitCargoPath
if ($cargoText -notmatch '(?m)^version = "([^"]+)"') {
    throw "Could not read the version from src-tauri\Cargo.toml"
}
$cargoVersion = $Matches[1]
if ($gitCargoText -notmatch '(?m)^version = "([^"]+)"') {
    throw "Could not read the version from crates\git\Cargo.toml"
}
$gitVersion = $Matches[1]
$version = [string]$tauri.version
if ($version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version must be three numbers, like 1.0.0. Got $version"
}
if ($version -ne [string]$package.version -or $version -ne $cargoVersion -or $version -ne $gitVersion) {
    throw "Version mismatch: tauri.conf.json=$version package.json=$($package.version) Cargo.toml=$cargoVersion crates/git=$gitVersion"
}

cmd /c "gh auth status >nul 2>nul"
if ($LASTEXITCODE -ne 0) {
    throw "gh is not logged in. Run gh auth login"
}
cmd /c "gh release view v$version --repo $repo >nul 2>nul"
if ($LASTEXITCODE -eq 0) {
    throw "GitHub release v$version already exists"
}

& (Join-Path $root "scripts\package-windows.ps1")
if ($LASTEXITCODE -ne 0) {
    throw "Windows packaging failed"
}

$bundle = Join-Path $root "target\release\bundle"
$nsis = Join-Path $bundle "nsis\AweGit_${version}_x64-setup.exe"
if (-not (Test-Path $nsis)) {
    throw "Missing installer $nsis"
}
$msiDir = Join-Path $bundle "msi"
$msi = $null
if (Test-Path $msiDir) {
    $msi = Get-ChildItem -Path $msiDir -Filter "AweGit_${version}_x64*.msi" | Select-Object -First 1
}

$tag = "v$version"
$fileName = [IO.Path]::GetFileName($nsis)
$download = "https://github.com/$repo/releases/download/$tag/$fileName"
$releaseNotes = if ($Notes) { $Notes } else { "AweGit $version" }
$assets = @($nsis)
if ($msi) { $assets += $msi.FullName }

gh release create $tag @assets --repo $repo --title "AweGit $version" --notes $releaseNotes
if ($LASTEXITCODE -ne 0) {
    throw "Could not create GitHub release $tag"
}

$latestPath = Join-Path $root "latest.json"
$appCode = 26
if (Test-Path $latestPath) {
    $previous = Read-JsonFile $latestPath
    if ($previous.appCode) {
        $appCode = [Math]::Max(26, [int]$previous.appCode + 1)
    }
}
$latest = [ordered]@{
    appVersion = $version
    notes = $Notes
    appCode = $appCode
    downloadUrl = $download
    downloads = [ordered]@{
        windows = $download
        macos = ""
    }
}
$json = ($latest | ConvertTo-Json -Depth 4) + "`n"
$utf8 = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($latestPath, $json, $utf8)

Push-Location $root
try {
    git add -- latest.json
    git commit -m "chore: 发布 $version"
    if ($LASTEXITCODE -ne 0) {
        throw "Could not commit latest.json"
    }
    git push origin release
    if ($LASTEXITCODE -ne 0) {
        throw "Could not push latest.json"
    }
}
finally {
    Pop-Location
}

Write-Host "Published $tag"
Write-Host $download
