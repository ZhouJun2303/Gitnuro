# Package AweGit for Windows, upload the installers, and publish the version manifest.
# Run this on the release branch. publish.bat merges feat/dev into release first.
# It commits latest.json, tags that commit, uploads the installers, then pushes origin release.

param(
    [string]$Notes = "",
    [string]$NotesMarkdown = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$repo = "ZhouJun2303/Gitnuro"
$stagePath = Join-Path $root ".git\awegit-publish-stage"
. (Join-Path $PSScriptRoot "gh-retry.ps1")
$publishWatch = [System.Diagnostics.Stopwatch]::StartNew()

function Set-PublishStage([string]$name) {
    $message = switch ($name) {
        "packaging" { "开始构建 Windows 安装包" }
        "released" { "GitHub release 已就绪，检查附件并准备推送版本清单" }
        "pushed" { "release 与版本清单已推送到 origin" }
    }
    if ($message) { Write-PublishLog $message }
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

function Get-NextAppCode {
    $latestPath = Join-Path $root "latest.json"
    $appCode = 26
    if (Test-Path $latestPath) {
        $previous = Read-JsonFile $latestPath
        if ($previous.appCode) {
            $appCode = [Math]::Max(26, [int]$previous.appCode + 1)
        }
    }
    return $appCode
}

function Write-LatestManifest([string]$version, [string]$notes, [string]$nsisPath, [int]$appCode) {
    Write-PublishLog "计算安装包 SHA-256 并生成 latest.json（版本 $version，appCode $appCode）"
    $latestPath = Join-Path $root "latest.json"
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
    Write-PublishLog "latest.json 已写入并回读核对，SHA-256：$sha256"
}

function Commit-Manifest([string]$version) {
    Write-PublishLog "提交 latest.json（版本 $version）"
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
        $commit = (& git -C $root rev-parse HEAD).Trim()
        Write-PublishLog "版本清单提交：$commit"
        return $commit
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
    return $assets
}

function Sync-ReleaseAssets([string]$tag, [string[]]$assets) {
    $listed = Invoke-GhRetry @("release", "view", $tag, "--repo", $repo, "--json", "assets", "--jq", ".assets[].name") "读取 release $tag 的附件"
    $have = @($listed -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    $missing = @($assets | Where-Object { $have -notcontains [IO.Path]::GetFileName($_) })
    Write-PublishLog "附件检查完成：本地 $($assets.Count) 个，待上传 $($missing.Count) 个"
    for ($i = 0; $i -lt $missing.Count; $i++) {
        $asset = Get-Item -LiteralPath $missing[$i]
        $what = "上传附件 $($i + 1)/$($missing.Count)：$($asset.Name)（{0:N1} MiB）" -f ($asset.Length / 1MB)
        Invoke-GhRetry @("release", "upload", $tag, $asset.FullName, "--repo", $repo, "--clobber") $what | Out-Null
    }
    Write-PublishLog "GitHub release $tag 的安装包附件已就绪"
}

function Publish-GitHubRelease([string]$tag) {
    $draft = Invoke-GhRetry @("release", "view", $tag, "--repo", $repo, "--json", "isDraft", "--jq", ".isDraft") "检查 release $tag 的公开状态"
    if ($draft.Trim() -eq "true") {
        Invoke-GhRetry @("release", "edit", $tag, "--repo", $repo, "--draft=false") "安装包已上传，公开 GitHub release $tag" | Out-Null
    } elseif ($draft.Trim() -ne "false") {
        throw "无法确认 GitHub release $tag 是否为草稿：$draft"
    }
    Write-PublishLog "GitHub release $tag 已公开"
}

# An earlier publish can leave the tag on a commit that never reached release.
# No GitHub release exists for that tag at this point, so it is safe to replace.
# A tag already inside release history is not ours to move.
function Reset-StaleTag([string]$tag) {
    Write-PublishLog "检查并清理上次未完成的标签 $tag"
    Push-Location $root
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & git -C $root tag -d $tag 2>$null | Out-Null
        $remote = Invoke-PublishProcess -Command git -Arguments @("-C", $root, "ls-remote", "--tags", "--refs", "origin", "refs/tags/$tag") -What "检查 origin 上的标签 $tag"
        if ($remote.Code -ne 0) { throw "Could not read tag $tag from origin：$($remote.Text)" }
        if (-not $remote.Stdout) { return }
        $fetched = Invoke-PublishProcess -Command git -Arguments @("-C", $root, "fetch", "--quiet", "origin", "release") -What "读取 origin/release，核对未完成的标签"
        if ($fetched.Code -ne 0) { throw "Could not fetch origin release：$($fetched.Text)" }
        $peek = "refs/awegit-stale/$tag"
        $fetched = Invoke-PublishProcess -Command git -Arguments @("-C", $root, "fetch", "--quiet", "--force", "origin", "refs/tags/${tag}:${peek}") -What "读取标签 $tag，核对其发布历史"
        if ($fetched.Code -ne 0) { throw "Could not fetch tag $tag：$($fetched.Text)" }
        & git -C $root merge-base --is-ancestor $peek origin/release 2>$null
        $inRelease = $LASTEXITCODE -eq 0
        & git -C $root update-ref -d $peek
        # Fetching a tag ref also recreates it under refs/tags locally.
        & git -C $root tag -d $tag 2>$null | Out-Null
        if ($inRelease) {
            throw "标签 $tag 指向 release 上已有的提交，但 GitHub 上没有对应的 release。先手动确认这个标签。"
        }
        Push-PublishRef $root ":refs/tags/$tag" "清理 origin 上未完成的标签 $tag"
        Write-PublishLog "已清理上次没有完成的标签 $tag"
    }
    finally {
        $ErrorActionPreference = $previous
        Pop-Location
    }
}

function New-ReleaseTag([string]$tag, [string]$commit, [string]$version) {
    Write-PublishLog "在版本清单提交 $commit 上创建标签 $tag"
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
    Push-PublishRef $root "refs/tags/$tag" "推送发布标签 $tag 到 origin"
}

function Push-Release {
    Push-PublishRef $root "refs/heads/release:refs/heads/release" "推送 release 与 latest.json 到 origin"
    Set-PublishStage "pushed"
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
Write-PublishLog "版本校验通过，准备发布 AweGit $version"

$bundle = Join-Path $root "target\release\bundle"
$nsis = Join-Path $bundle "nsis\AweGit_${version}_x64-setup.exe"
$tag = "v$version"
$releaseExists = Test-GitHubRelease -Tag $tag -Repo $repo

if ($releaseExists) {
    Write-PublishLog "GitHub 上已经有 $tag，补齐安装包并推送版本清单"
    Set-PublishStage "released"
    $assets = @(Get-InstallerAssets $version $nsis)
    if ($assets.Count -gt 0) {
        Sync-ReleaseAssets $tag $assets
    }
    if (-not (Test-ManifestReady $version)) {
        if (-not (Test-Path $nsis)) {
            throw "GitHub release $tag already exists, and latest.json does not point at it"
        }
        Write-LatestManifest $version $Notes $nsis (Get-NextAppCode)
    }
} else {
    Set-PublishStage "packaging"
    $appCode = Get-NextAppCode
    # build.rs embeds this code in the About panel; the manifest below writes the same number.
    $env:AWE_APP_CODE = "$appCode"
    & (Join-Path $root "scripts\package-windows.ps1")
    if ($LASTEXITCODE -ne 0) {
        throw "Windows packaging failed"
    }
    if (-not (Test-Path $nsis)) {
        throw "Missing installer $nsis"
    }
    Write-PublishLog "Windows 安装包构建完成"
    $assets = @(Get-InstallerAssets $version $nsis)
    foreach ($assetPath in $assets) {
        $asset = Get-Item -LiteralPath $assetPath
        Write-PublishLog ("待发布附件：$($asset.Name)（{0:N1} MiB）" -f ($asset.Length / 1MB))
    }
    $releaseNotes = if ($Notes) { $Notes } else { "AweGit $version" }
    # latest.json shows the plain lines in the app; GitHub renders the Markdown.
    $releaseBody = if ($NotesMarkdown) { $NotesMarkdown } else { $releaseNotes }
    # Multi-line notes cannot survive the Windows command line; gh reads them from a file.
    $notesFile = Join-Path $env:TEMP ("awegit-notes-" + [guid]::NewGuid().ToString("N") + ".txt")
    [IO.File]::WriteAllText($notesFile, $releaseBody, (New-Object System.Text.UTF8Encoding $false))
    $tagPushed = $false
    try {
        # The manifest commit is tagged before the release exists, so the tag
        # names the exact commit release will end up at. Letting gh create the
        # tag would put it on origin release, still the previous publish.
        Write-LatestManifest $version $Notes $nsis $appCode
        $commit = Commit-Manifest $version
        Reset-StaleTag $tag
        New-ReleaseTag $tag $commit $version
        Push-ReleaseTag $tag
        $tagPushed = $true
        # Keep incomplete uploads in a draft; publish only after all installers
        # are ready. Each file has its own progress and retry logs.
        $createArgs = @("release", "create", $tag, "--draft", "--verify-tag", "--repo", $repo, "--title", "AweGit $version", "--notes-file", $notesFile)
        $created = $false
        $lastText = ""
        for ($attempt = 1; $attempt -le 6 -and -not $created; $attempt++) {
            $result = Invoke-GhOnce $createArgs "创建 GitHub release $tag（第 $attempt/6 次）"
            if ($result.Code -eq 0) { $created = $true; break }
            $lastText = $result.Text
            # The request can time out after GitHub created the release.
            if (Test-GitHubRelease -Tag $tag -Repo $repo) { $created = $true; break }
            if ($attempt -lt 6) {
                Write-PublishLog "创建 GitHub release $tag 没有成功，正在重试 ($attempt)"
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
        # Also resumes an earlier upload that only completed some installers.
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

Publish-GitHubRelease $tag
Push-Release
$published = Read-JsonFile (Join-Path $root "latest.json")
Write-PublishLog ("AweGit v$version 发布完成，总耗时 {0:N1} 秒" -f $publishWatch.Elapsed.TotalSeconds)
Write-Host ([string]$published.downloadUrl)
