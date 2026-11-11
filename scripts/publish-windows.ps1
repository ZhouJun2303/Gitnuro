# Package AweGit for Windows, upload the installers, and publish the version manifest.
# Run this on the release branch. publish.bat merges feat/dev into release first.
# It commits latest.json, tags that commit, uploads the installers, then pushes origin release.

param(
    [string]$Notes = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$repo = "ZhouJun2303/Gitnuro"
$stagePath = Join-Path $root ".git\awegit-publish-stage"
. (Join-Path $PSScriptRoot "gh-retry.ps1")

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

function Commit-Manifest([string]$version) {
    Push-Location $root
    try {
        git add -- latest.json | Out-Null
        $staged = git diff --cached --name-only -- latest.json
        if ($staged) {
            git commit -m "chore: 发布 $version" | Out-Null
            if ($LASTEXITCODE -ne 0) {
                throw "Could not commit latest.json"
            }
        }
        return (& git -C $root rev-parse HEAD).Trim()
    }
    finally {
        Pop-Location
    }
}

function Get-InstallerAssets([string]$version, [string]$nsis) {
    $assets = @()
    if (Test-Path $nsis) { $assets += $nsis }
    $msiDir = Join-Path $bundle "msi"
    if (Test-Path $msiDir) {
        $msi = Get-ChildItem -Path $msiDir -Filter "AweGit_${version}_x64*.msi" | Select-Object -First 1
        if ($msi) { $assets += $msi.FullName }
    }
    return ,$assets
}

function Sync-ReleaseAssets([string]$tag, [string[]]$assets) {
    $listed = Invoke-GhRetry @("release", "view", $tag, "--repo", $repo, "--json", "assets", "--jq", ".assets[].name") "读取 release $tag 的附件"
    $have = @($listed -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    $missing = @($assets | Where-Object { $have -notcontains [IO.Path]::GetFileName($_) })
    if ($missing.Count -gt 0) {
        Invoke-GhRetry (@("release", "upload", $tag) + $missing + @("--repo", $repo, "--clobber")) "上传 release 附件" | Out-Null
    }
}

# An earlier publish can leave the tag on a commit that never reached release.
# No GitHub release exists for that tag at this point, so it is safe to replace.
# A tag already inside release history is not ours to move.
function Reset-StaleTag([string]$tag) {
    Push-Location $root
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & git -C $root tag -d $tag 2>$null | Out-Null
        $remote = (& git -C $root ls-remote --tags --refs origin "refs/tags/$tag" 2>$null | Out-String).Trim()
        if ($LASTEXITCODE -ne 0) { throw "Could not read tag $tag from origin" }
        if (-not $remote) { return }
        & git -C $root fetch --quiet origin release 2>$null
        if ($LASTEXITCODE -ne 0) { throw "Could not fetch origin release" }
        $peek = "refs/awegit-stale/$tag"
        & git -C $root fetch --quiet --force origin "refs/tags/${tag}:${peek}" 2>$null
        if ($LASTEXITCODE -ne 0) { throw "Could not fetch tag $tag" }
        & git -C $root merge-base --is-ancestor $peek origin/release 2>$null
        $inRelease = $LASTEXITCODE -eq 0
        & git -C $root update-ref -d $peek
        # Fetching a tag ref also recreates it under refs/tags locally.
        & git -C $root tag -d $tag 2>$null | Out-Null
        if ($inRelease) {
            throw "标签 $tag 指向 release 上已有的提交，但 GitHub 上没有对应的 release。先手动确认这个标签。"
        }
        & git -c lfs.locksverify=false push origin ":refs/tags/$tag" 2>$null
        if ($LASTEXITCODE -ne 0) { throw "Could not remove stale tag $tag from origin" }
        Write-Host "清理上次没有完成的标签 $tag"
    }
    finally {
        $ErrorActionPreference = $previous
        Pop-Location
    }
}

function New-ReleaseTag([string]$tag, [string]$commit, [string]$version) {
    Push-Location $root
    try {
        & git -C $root tag -a $tag $commit -m "AweGit $version"
        if ($LASTEXITCODE -ne 0) { throw "Could not create tag $tag" }
    }
    finally {
        Pop-Location
    }
}

function Push-ReleaseTag([string]$tag) {
    Push-Location $root
    try {
        $pushed = $false
        foreach ($attempt in 1..3) {
            git -c lfs.locksverify=false push origin "refs/tags/$tag"
            if ($LASTEXITCODE -eq 0) { $pushed = $true; break }
            if ($attempt -lt 3) {
                Write-Host "推送标签 $tag 没有成功，正在重试 ($attempt)"
                Start-Sleep -Seconds (5 * $attempt)
            }
        }
        if (-not $pushed) {
            throw "Could not push tag $tag"
        }
    }
    finally {
        Pop-Location
    }
}

function Push-Release {
    Push-Location $root
    try {
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

Assert-GhAuthenticated

$bundle = Join-Path $root "target\release\bundle"
$nsis = Join-Path $bundle "nsis\AweGit_${version}_x64-setup.exe"
$tag = "v$version"
$releaseExists = Test-GitHubRelease -Tag $tag -Repo $repo

if ($releaseExists) {
    Write-Host "GitHub 上已经有 $tag，改为推送版本清单"
    Set-PublishStage "released"
    $assets = @(Get-InstallerAssets $version $nsis)
    if ($assets.Count -gt 0) {
        Sync-ReleaseAssets $tag $assets
    }
    if (-not (Test-ManifestReady $version)) {
        if (-not (Test-Path $nsis)) {
            throw "GitHub release $tag already exists, and latest.json does not point at it"
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
    $assets = @(Get-InstallerAssets $version $nsis)
    $releaseNotes = if ($Notes) { $Notes } else { "AweGit $version" }
    # Multi-line notes cannot survive the Windows command line; gh reads them from a file.
    $notesFile = Join-Path $env:TEMP ("awegit-notes-" + [guid]::NewGuid().ToString("N") + ".txt")
    [IO.File]::WriteAllText($notesFile, $releaseNotes, (New-Object System.Text.UTF8Encoding $false))
    $tagPushed = $false
    try {
        # The manifest commit is tagged before the release exists, so the tag
        # names the exact commit release will end up at. Letting gh create the
        # tag would put it on origin release, still the previous publish.
        Write-LatestManifest $version $Notes $nsis
        $commit = Commit-Manifest $version
        Reset-StaleTag $tag
        New-ReleaseTag $tag $commit $version
        Push-ReleaseTag $tag
        $tagPushed = $true
        $createArgs = @("release", "create", $tag) + $assets + @("--verify-tag", "--repo", $repo, "--title", "AweGit $version", "--notes-file", $notesFile)
        $created = $false
        $lastText = ""
        for ($attempt = 1; $attempt -le 6 -and -not $created; $attempt++) {
            $result = Invoke-GhOnce $createArgs
            if ($result.Code -eq 0) { $created = $true; break }
            $lastText = $result.Text
            # The request can time out after GitHub created the release.
            if (Test-GitHubRelease -Tag $tag -Repo $repo) { $created = $true; break }
            if ($attempt -lt 6) {
                Write-Host "创建 GitHub release $tag 没有成功，正在重试 ($attempt)"
                Start-Sleep -Seconds ([Math]::Min(3 * $attempt, 15))
            }
        }
        if (-not $created) {
            throw "Could not create GitHub release $tag：$lastText"
        }
        # From here the tag and release are live: keep the local manifest
        # commit so a retry pushes the same commit and the tag stays on release.
        Set-PublishStage "released"
        $tagPushed = $false
        # A create that timed out midway can leave the release without its
        # installers; upload whatever is still missing.
        Sync-ReleaseAssets $tag $assets
    }
    catch {
        if ($tagPushed) {
            # The release was never created, so drop the local tag; the retry
            # then tags its fresh manifest commit instead.
            & git -C $root tag -d $tag 2>$null | Out-Null
        }
        throw
    }
    finally {
        Remove-Item -LiteralPath $notesFile -Force -ErrorAction SilentlyContinue
    }
}

Push-Release
$published = Read-JsonFile (Join-Path $root "latest.json")
Write-Host "Published v$version"
Write-Host ([string]$published.downloadUrl)
