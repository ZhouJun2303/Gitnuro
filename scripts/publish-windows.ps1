# Package AweGit for Windows, upload the installers, and publish the version manifest.
# Run this on the release branch. publish.bat merges feat/dev into release first.
# It commits latest.json at the repo root and pushes origin release.

param(
    [string]$Notes = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$repo = "ZhouJun2303/Gitnuro"
$stagePath = Join-Path $root ".git\awegit-publish-stage"

function Set-PublishStage([string]$name) {
    if (-not (Test-Path (Split-Path $stagePath))) { return }
    [System.IO.File]::WriteAllText($stagePath, $name)
}

Push-Location $root
try {
    $current = (git rev-parse --abbrev-ref HEAD).Trim()
    if ($current -ne "release") {
        throw "Publish from the release branch; publish.bat merges feat/dev there first. Current branch is $current"
    }
}
finally {
    Pop-Location
}

function Read-JsonFile([string]$path) {
    Get-Content -Raw -Encoding UTF8 -Path $path | ConvertFrom-Json
}

function Test-ManifestReady([string]$version) {
    $latestPath = Join-Path $root "latest.json"
    if (-not (Test-Path $latestPath)) { return $false }
    $written = Read-JsonFile $latestPath
    $download = [string]$written.downloadUrl
    $ver = [regex]::Escape($version)
    return ([string]$written.appVersion -eq $version -and $download -match "/releases/download/v$ver/")
}

function Write-LatestManifest([string]$version, [string]$notes, [string]$nsisPath) {
    $latestPath = Join-Path $root "latest.json"
    $appCode = 26
    if (Test-Path $latestPath) {
        $previous = Read-JsonFile $latestPath
        if ($previous.appCode) {
            $appCode = [Math]::Max(26, [int]$previous.appCode + 1)
        }
    }
    $sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $nsisPath).Hash.ToLower()
    $fileName = [IO.Path]::GetFileName($nsisPath)
    $download = "https://github.com/$repo/releases/download/v$version/$fileName"
    $latest = [ordered]@{
        appVersion = $version
        notes = $notes
        appCode = $appCode
        downloadUrl = $download
        downloads = [ordered]@{
            windows = [ordered]@{ url = $download; sha256 = $sha256 }
            macos = ""
        }
    }
    $json = ($latest | ConvertTo-Json -Depth 4) + "`n"
    $utf8 = New-Object System.Text.UTF8Encoding $false
    $latestTemp = "$latestPath.tmp"
    [System.IO.File]::WriteAllText($latestTemp, $json, $utf8)
    Move-Item -LiteralPath $latestTemp -Destination $latestPath -Force
    $written = Get-Content -Raw -Encoding UTF8 -Path $latestPath | ConvertFrom-Json
    if ([string]$written.appVersion -ne $version) {
        throw "latest.json version is $($written.appVersion), expected $version"
    }
}

function Submit-Manifest([string]$version) {
    Push-Location $root
    try {
        git add -- latest.json
        $staged = git diff --cached --name-only -- latest.json
        if ($staged) {
            git commit -m "chore: 发布 $version"
            if ($LASTEXITCODE -ne 0) {
                throw "Could not commit latest.json"
            }
        }
        # Nothing in this repo is stored in Git LFS. The pre-push hook still
        # POSTs /info/lfs/locks/verify, and GitHub often closes that call with
        # EOF, which rejects the push after the installer is already uploaded.
        $pushed = $false
        foreach ($attempt in 1..3) {
            git -c lfs.locksverify=false push origin release
            if ($LASTEXITCODE -eq 0) { $pushed = $true; break }
            if ($attempt -lt 3) {
                Write-Host "推送 release 没有成功，正在重试 ($attempt)"
                Start-Sleep -Seconds (5 * $attempt)
            }
        }
        if (-not $pushed) {
            throw "Could not push latest.json"
        }
        Set-PublishStage "pushed"
    }
    finally {
        Pop-Location
    }
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
$versionCheck = & node (Join-Path $root "scripts\check-versions.mjs") 2>&1
if ($LASTEXITCODE -ne 0) {
    throw ($versionCheck | Out-String)
}
if ($version -ne [string]$package.version -or $version -ne $cargoVersion -or $version -ne $gitVersion) {
    throw "Version mismatch: tauri.conf.json=$version package.json=$($package.version) Cargo.toml=$cargoVersion crates/git=$gitVersion"
}

cmd /c "gh auth status >nul 2>nul"
if ($LASTEXITCODE -ne 0) {
    throw "gh is not logged in. Run gh auth login"
}

$bundle = Join-Path $root "target\release\bundle"
$nsis = Join-Path $bundle "nsis\AweGit_${version}_x64-setup.exe"
cmd /c "gh release view v$version --repo $repo >nul 2>nul"
$releaseExists = ($LASTEXITCODE -eq 0)

if ($releaseExists) {
    Write-Host "GitHub 上已经有 v$version，改为推送版本清单"
    Set-PublishStage "released"
    if (-not (Test-ManifestReady $version)) {
        if (-not (Test-Path $nsis)) {
            throw "GitHub release v$version already exists, and latest.json does not point at it"
        }
        Write-LatestManifest $version $Notes $nsis
    }
} else {
    Set-PublishStage "packaging"
    & (Join-Path $root "scripts\package-windows.ps1")
    if ($LASTEXITCODE -ne 0) {
        throw "Windows packaging failed"
    }
    if (-not (Test-Path $nsis)) {
        throw "Missing installer $nsis"
    }
    $msiDir = Join-Path $bundle "msi"
    $msi = $null
    if (Test-Path $msiDir) {
        $msi = Get-ChildItem -Path $msiDir -Filter "AweGit_${version}_x64*.msi" | Select-Object -First 1
    }
    $tag = "v$version"
    $releaseNotes = if ($Notes) { $Notes } else { "AweGit $version" }
    $assets = @($nsis)
    if ($msi) { $assets += $msi.FullName }
    gh release create $tag @assets --repo $repo --title "AweGit $version" --notes $releaseNotes
    if ($LASTEXITCODE -ne 0) {
        throw "Could not create GitHub release $tag"
    }
    Set-PublishStage "released"
    Write-LatestManifest $version $Notes $nsis
}

Submit-Manifest $version
$published = Read-JsonFile (Join-Path $root "latest.json")
Write-Host "Published v$version"
Write-Host ([string]$published.downloadUrl)
