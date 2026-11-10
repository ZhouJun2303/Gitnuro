# Interactive Windows publish menu.
# Bumps the patch, minor, or major version, then either packages only or publishes to GitHub.

param(
    [string]$Action = "",
    [string]$Version = "",
    [string]$Notes = "",
    [switch]$Yes
)

$ErrorActionPreference = "Stop"
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -Scope Global -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$repo = "ZhouJun2303/Gitnuro"
$devBranch = "feat/dev"

function Invoke-Git {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$GitArgs)
    & git -C $root @GitArgs
    if ($LASTEXITCODE -ne 0) {
        throw "git $($GitArgs -join ' ') failed"
    }
}

function Get-JsonVersion([string]$text) {
    $match = [regex]::Match($text, '(?m)^\s*"version"\s*:\s*"(\d+\.\d+\.\d+)"')
    if (-not $match.Success) { throw "Could not find a JSON version" }
    return $match.Groups[1].Value
}

function Update-FirstMatch([string]$text, [regex]$pattern, [string]$replacement) {
    $match = $pattern.Match($text)
    if (-not $match.Success) { throw "Version pattern was not found" }
    return $text.Remove($match.Index, $match.Length).Insert($match.Index, $replacement)
}

function Get-ProjectVersion {
    $output = & node (Join-Path $root "scripts\check-versions.mjs") 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw ($output | Out-String)
    }
    return Get-JsonVersion ([IO.File]::ReadAllText((Join-Path $root "package.json")))
}

function ConvertTo-Parts([string]$text) {
    if ($text -notmatch '^(\d+)\.(\d+)\.(\d+)$') {
        throw "版本号必须是 1.0.0 这种三段数字，当前是 $text"
    }
    return @([int]$Matches[1], [int]$Matches[2], [int]$Matches[3])
}

function Get-BumpedVersion([string]$current, [string]$part) {
    $numbers = ConvertTo-Parts $current
    switch ($part) {
        "patch" { $numbers[2]++ }
        "minor" { $numbers[1]++; $numbers[2] = 0 }
        "major" { $numbers[0]++; $numbers[1] = 0; $numbers[2] = 0 }
        default { throw "Unknown bump $part" }
    }
    return "$($numbers[0]).$($numbers[1]).$($numbers[2])"
}

function Test-VersionGreater([string]$left, [string]$right) {
    $a = ConvertTo-Parts $left
    $b = ConvertTo-Parts $right
    for ($i = 0; $i -lt 3; $i++) {
        if ($a[$i] -gt $b[$i]) { return $true }
        if ($a[$i] -lt $b[$i]) { return $false }
    }
    return $false
}

function Set-ProjectVersion([string]$version) {
    $utf8 = New-Object System.Text.UTF8Encoding $false
    $previous = Get-JsonVersion ([IO.File]::ReadAllText((Join-Path $root "package.json")))
    $jsonPattern = [regex]'(?m)^(\s*"version"\s*:\s*")\d+\.\d+\.\d+(")'
    $tomlPattern = [regex]'(?m)^version = "\d+\.\d+\.\d+"'
    $files = @(
        @{ Path = (Join-Path $root "package.json"); Kind = "json" },
        @{ Path = (Join-Path $root "src-tauri\tauri.conf.json"); Kind = "json" },
        @{ Path = (Join-Path $root "src-tauri\Cargo.toml"); Kind = "toml" },
        @{ Path = (Join-Path $root "crates\git\Cargo.toml"); Kind = "toml" }
    )
    $pending = @()
    foreach ($file in $files) {
        $text = [IO.File]::ReadAllText($file.Path)
        if ($file.Kind -eq "json") {
            $match = $jsonPattern.Match($text)
            if (-not $match.Success) { throw "Could not update $($file.Path)" }
            $updated = $text.Remove($match.Index, $match.Length).Insert($match.Index, $match.Groups[1].Value + $version + $match.Groups[2].Value)
        } else {
            $updated = Update-FirstMatch $text $tomlPattern "version = `"$version`""
        }
        $pending += @{ Path = $file.Path; Text = $updated }
    }
    $lockPath = Join-Path $root "Cargo.lock"
    $lock = [IO.File]::ReadAllText($lockPath)
    foreach ($name in @("awegit-git", "awegit")) {
        $pattern = [regex]"(?s)(\[\[package\]\]\r?\nname = `"$name`"\r?\nversion = `")\d+\.\d+\.\d+"
        $match = $pattern.Match($lock)
        if (-not $match.Success) { throw "Could not update Cargo.lock package $name" }
        $lock = $lock.Remove($match.Index, $match.Length).Insert($match.Index, $match.Groups[1].Value + $version)
    }
    $pending += @{ Path = $lockPath; Text = $lock }

    $readmePath = Join-Path $root "README.md"
    $readme = [IO.File]::ReadAllText($readmePath)
    $readmeUpdated = $readme.Replace("当前版本是 $previous", "当前版本是 $version").Replace("AweGit_${previous}_", "AweGit_${version}_")
    if ($previous -ne $version -and $readmeUpdated -eq $readme) { throw "Could not update the version in README.md" }
    $pending += @{ Path = $readmePath; Text = $readmeUpdated }

    $statePath = Join-Path $root "src\lib\shell\state.svelte.ts"
    $state = [IO.File]::ReadAllText($statePath)
    $stateOld = 'appVersion = $state("' + $previous + '")'
    $stateNew = 'appVersion = $state("' + $version + '")'
    if (-not $state.Contains($stateOld)) { throw "Could not update the about-page version" }
    $pending += @{ Path = $statePath; Text = $state.Replace($stateOld, $stateNew) }

    $latestPath = Join-Path $root "latest.json"
    $latest = [IO.File]::ReadAllText($latestPath)
    $latestPattern = [regex]'"appVersion"\s*:\s*"\d+\.\d+\.\d+"'
    if (-not $latestPattern.IsMatch($latest)) { throw "Could not update latest.json" }
    $pending += @{ Path = $latestPath; Text = $latestPattern.Replace($latest, ('"appVersion": "' + $version + '"'), 1) }

    foreach ($item in $pending) {
        [IO.File]::WriteAllText($item.Path, $item.Text, $utf8)
    }
}

function Assert-Clean {
    $merge = & git -C $root rev-parse -q --verify MERGE_HEAD 2>$null
    if ($LASTEXITCODE -eq 0 -and $merge) { throw "有一次合并还没完成" }
    $status = @(& git -C $root status --porcelain | Where-Object { $_ })
    if (-not $status) { return }
    foreach ($line in $status) {
        if ($line -match '^\?\?') {
            throw "工作区还有未提交的改动，先提交或暂存。`n$($status -join "`n")"
        }
    }
    # A build can touch generated files without changing their bytes.
    # status still lists them; diff does not. Those are not unpublished work.
    $work = & git -C $root diff --raw
    $index = & git -C $root diff --cached --raw
    if ($work -or $index) {
        throw "工作区还有未提交的改动，先提交或暂存。`n$($status -join "`n")"
    }
}

function Test-ManifestPublished([string]$version) {
    & git -C $root fetch origin release
    if ($LASTEXITCODE -ne 0) { throw "没能读取 origin/release" }
    $json = & git -C $root show "origin/release:latest.json" 2>$null
    if ($LASTEXITCODE -ne 0) { return $false }
    $text = ($json | Out-String)
    $ver = [regex]::Escape($version)
    return ($text -match ('"appVersion"\s*:\s*"' + $ver + '"') -and $text -match ('releases/download/v' + $ver + '/'))
}

function Get-UnmergedPaths {
    @(& git -C $root diff --name-only --diff-filter=U | ForEach-Object { "$_".Trim() } | Where-Object { $_ })
}

function Stop-OpenMerge {
    $merging = & git -C $root rev-parse -q --verify MERGE_HEAD 2>$null
    if ($LASTEXITCODE -eq 0 -and $merging) {
        & git -C $root merge --abort 2>$null
    }
}

# release keeps the published manifest (download URL, notes, appCode).
# The branch being merged only has a version stub, so latest.json conflicts on every publish.
# Keep release's file and move appVersion to the version about to ship.
# publish-windows.ps1 rewrites the download fields after the installer is uploaded.
function Restore-PublishedManifest([string]$version) {
    & git -C $root checkout --ours -- latest.json
    if ($LASTEXITCODE -ne 0) { throw "git checkout --ours -- latest.json failed" }
    $path = Join-Path $root "latest.json"
    $text = [IO.File]::ReadAllText($path)
    $pattern = [regex]'"appVersion"\s*:\s*"\d+\.\d+\.\d+"'
    if (-not $pattern.IsMatch($text)) { throw "Could not update latest.json after merge" }
    $utf8 = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $pattern.Replace($text, ('"appVersion": "' + $version + '"'), 1), $utf8)
    & git -C $root add -- latest.json
    if ($LASTEXITCODE -ne 0) { throw "git add -- latest.json failed" }
}

function Get-CurrentBranch {
    $branch = (& git -C $root branch --show-current).Trim()
    if (-not $branch) { throw "当前不在任何分支上" }
    return $branch
}

# Everything on this branch that the last publish did not ship yet.
# origin/release is the boundary, not the v* tags: gh creates the tag
# before the release branch is pushed, so tags land on the previous manifest commit.
function Get-ChangelogNotes {
    # Best-effort refresh: a flaky network must not abort the publish; fall back
    # to the existing ref. Native stderr would be a terminating error under Stop.
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & git -C $root fetch --quiet origin release 2>$null | Out-Null
        $base = $null
        & git -C $root rev-parse -q --verify "origin/release" 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) {
            $base = "origin/release"
        } else {
            $tag = & git -C $root describe --tags --abbrev=0 --match "v*" 2>$null
            if ($LASTEXITCODE -eq 0 -and $tag) { $base = "$tag".Trim() }
        }
        $logArgs = @("-C", $root, "log", "--no-merges", "--pretty=format:%s")
        if ($base) { $logArgs += "$base..HEAD" } else { $logArgs += @("-n", "50") }
        $subjects = @(& git @logArgs 2>$null)
    } finally {
        $ErrorActionPreference = $previous
    }

    $labelByType = @{
        feat = "新功能"; fix = "修复"; perf = "性能"; style = "界面"; art = "界面"
        refactor = "重构"; docs = "文档"; test = "测试"
    }
    $order = @("新功能", "修复", "性能", "界面", "重构", "文档", "测试", "其他")
    $groups = @{}
    foreach ($label in $order) { $groups[$label] = New-Object System.Collections.Generic.List[string] }
    foreach ($subject in $subjects) {
        $text = "$subject".Trim()
        if (-not $text) { continue }
        if ($text -match '^chore:\s*(版本|发布|合并)\s*') { continue }
        $label = "其他"
        $item = $text
        if ($text -match '^(feat|fix|docs|style|refactor|test|chore|perf|art|revert)(\([^)]*\))?:\s*(.+)$') {
            $type = $Matches[1]
            $item = $Matches[3].Trim()
            if ($labelByType.ContainsKey($type)) { $label = $labelByType[$type] }
        }
        $groups[$label].Add($item)
    }
    $parts = @()
    foreach ($label in $order) {
        $items = $groups[$label]
        if ($items.Count -eq 0) { continue }
        $parts += ($label + "：" + ($items -join "、"))
    }
    if ($parts.Count -eq 0) { return "" }
    return ("本次更新 " + ($parts -join "；") + "。")
}

function Assert-ReleaseAvailable([string]$version) {
    cmd /c "gh auth status >nul 2>nul"
    if ($LASTEXITCODE -ne 0) { throw "gh 还没登录。先运行 gh auth login" }
    cmd /c "gh release view v$version --repo $repo >nul 2>nul"
    if ($LASTEXITCODE -eq 0 -and (Test-ManifestPublished $version)) {
        throw "GitHub 上已经有 v$version，请把版本号加高"
    }
}

function Invoke-ReleaseFlow {
    param(
        [string]$Bump = "",
        [string]$SetVersion = "",
        [switch]$Publish,
        [string]$ReleaseNotes = "",
        [switch]$Confirm
    )

    Assert-Clean
    $current = Get-ProjectVersion
    $target = $current
    if ($Bump) { $target = Get-BumpedVersion $current $Bump }
    if ($SetVersion) { $target = $SetVersion }
    ConvertTo-Parts $target | Out-Null
    if ($target -ne $current -and -not (Test-VersionGreater $target $current)) {
        throw "新版本 $target 必须高于当前版本 $current"
    }
    if ($Publish) { Assert-ReleaseAvailable $target }

    $branch = Get-CurrentBranch
    Write-Host ""
    if ($target -eq $current) {
        Write-Host "版本号保持 $current"
    } else {
        Write-Host "版本号 $current -> $target"
        Write-Host "会在 $branch 上提交这个版本号"
    }
    if ($Publish) {
        if ($branch -eq "release") {
            Write-Host "在 release 上打包并上传"
        } elseif ($branch -eq $devBranch) {
            Write-Host "把 $devBranch 合并进 release，打包并上传，完成后回到 $devBranch"
        } else {
            Write-Host "当前不在 $devBranch 或 release 上。会把 $branch 合并进 release，打包并上传，完成后回到 $branch"
        }
        if (-not $ReleaseNotes) {
            $ReleaseNotes = Get-ChangelogNotes
            Write-Host ""
            if ($ReleaseNotes) {
                Write-Host "更新说明（按上个发布版本以来的提交自动生成）"
                Write-Host $ReleaseNotes
            } else {
                Write-Host "没找到上次发布之后的提交，更新说明用 AweGit $target"
            }
        } else {
            Write-Host "说明：$ReleaseNotes"
        }
    } else {
        Write-Host "只打包，不上传"
    }

    if (-not $Confirm) {
        $answer = Read-Host "输入 y 开始"
        if ($answer -ne "y" -and $answer -ne "Y") {
            Write-Host "已取消"
            return $false
        }
    }

    if ($target -ne $current) {
        Set-ProjectVersion $target
        Invoke-Git add -- package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml crates/git/Cargo.toml Cargo.lock README.md latest.json src/lib/shell/state.svelte.ts
        Invoke-Git commit -m "chore: 版本 $target"
        Write-Host "已提交版本 $target"
    }

    $switched = $false
    $releaseBefore = $null
    try {
        if ($Publish -and $branch -ne "release") {
            Write-Host "切换到 release，并合并 $branch"
            Invoke-Git checkout release
            $switched = $true
            $releaseBefore = (& git -C $root rev-parse HEAD).Trim()
            & git -C $root merge -m "chore: 合并 $branch" $branch
            if ($LASTEXITCODE -ne 0) {
                $names = @(Get-UnmergedPaths)
                $onlyLatest = $names.Count -eq 1 -and $names[0] -eq "latest.json"
                if ($onlyLatest) {
                    try {
                        Restore-PublishedManifest $target
                        & git -C $root commit -m "chore: 合并 $branch"
                        if ($LASTEXITCODE -ne 0) { throw "git commit failed" }
                    } catch {
                        Stop-OpenMerge
                        throw "没能把 $branch 合并进 release。版本号提交还留在 $branch 上。$($_.Exception.Message)"
                    }
                    Write-Host "latest.json 保留 release 上已发布的下载地址和 appCode，版本号改为 $target"
                } else {
                    Stop-OpenMerge
                    $list = if ($names.Count) { $names -join "、" } else { "没有列出冲突文件" }
                    throw "没能把 $branch 合并进 release。版本号提交还留在 $branch 上。`n$list"
                }
            }
        }
        if ($Publish) {
            $stagePath = Join-Path $root ".git\awegit-publish-stage"
            if (Test-Path $stagePath) { Remove-Item $stagePath -Force }
            try {
                & (Join-Path $root "scripts\publish-windows.ps1") -Notes $ReleaseNotes
                if ($LASTEXITCODE -ne 0) { throw "发布失败" }
            } catch {
                $stage = ""
                if (Test-Path $stagePath) { $stage = (Get-Content -Raw $stagePath).Trim() }
                if ($switched -and $releaseBefore -and $stage -ne "released" -and $stage -ne "pushed") {
                    & git -C $root reset --hard $releaseBefore
                    if ($LASTEXITCODE -ne 0) {
                        Write-Host "没能撤回 release 上的合并。"
                    } else {
                        Write-Host "发布没有完成，已撤回 release 上这次合并。"
                    }
                } elseif ($stage -eq "released") {
                    Write-Host "安装包已经上传，版本清单还没有推到 release。再发布一次当前版本即可，不要改版本号。"
                }
                throw
            } finally {
                if (Test-Path $stagePath) { Remove-Item $stagePath -Force }
            }
        } else {
            & (Join-Path $root "scripts\package-windows.ps1")
            if ($LASTEXITCODE -ne 0) { throw "打包失败" }
        }
    } finally {
        if ($switched) {
            & git -C $root checkout $branch
            if ($LASTEXITCODE -ne 0) {
                Write-Host "没有回到 $branch。请先看一下当前分支。"
            }
        }
    }
    return $true
}

function Show-Menu {
    $current = Get-ProjectVersion
    $branch = Get-CurrentBranch
    $patch = Get-BumpedVersion $current "patch"
    $minor = Get-BumpedVersion $current "minor"
    $major = Get-BumpedVersion $current "major"
    Write-Host ""
    Write-Host "AweGit 发布"
    Write-Host "当前版本 $current"
    Write-Host "当前分支 $branch"
    Write-Host "日常开发 $devBranch，发布 release"
    Write-Host ""
    Write-Host "1  修订号 +1 并发布到 GitHub    $current -> $patch"
    Write-Host "2  次版本 +1 并发布到 GitHub    $current -> $minor"
    Write-Host "3  主版本 +1 并发布到 GitHub    $current -> $major"
    Write-Host "4  指定版本号并发布"
    Write-Host "5  不改版本，直接发布 $current"
    Write-Host "6  修订号 +1，只打包不上传"
    Write-Host "7  不改版本，只打包不上传"
    Write-Host "0  退出"
    Write-Host ""
}

if ($MyInvocation.InvocationName -eq ".") { return }

switch ($Action) {
    "1" { $Action = "patch" }
    "2" { $Action = "minor" }
    "3" { $Action = "major" }
    "4" { $Action = "set" }
    "5" { $Action = "current" }
    "6" { $Action = "package-patch" }
    "7" { $Action = "package" }
}

if (-not $Action -or $Action -eq "menu") {
    while ($true) {
        Show-Menu
        $choice = Read-Host "请选择"
        $done = $false
        switch ($choice) {
            "1" { $done = Invoke-ReleaseFlow -Bump patch -Publish }
            "2" { $done = Invoke-ReleaseFlow -Bump minor -Publish }
            "3" { $done = Invoke-ReleaseFlow -Bump major -Publish }
            "4" {
                $entered = Read-Host "版本号，例如 2.1.0"
                $done = Invoke-ReleaseFlow -SetVersion $entered.Trim() -Publish
            }
            "5" { $done = Invoke-ReleaseFlow -Publish }
            "6" { $done = Invoke-ReleaseFlow -Bump patch }
            "7" { $done = Invoke-ReleaseFlow }
            "0" { exit 0 }
            default { Write-Host "没有这个选项" }
        }
        if ($done) { exit 0 }
    }
}

switch ($Action) {
    "patch" { Invoke-ReleaseFlow -Bump patch -Publish -ReleaseNotes $Notes -Confirm:$Yes | Out-Null }
    "minor" { Invoke-ReleaseFlow -Bump minor -Publish -ReleaseNotes $Notes -Confirm:$Yes | Out-Null }
    "major" { Invoke-ReleaseFlow -Bump major -Publish -ReleaseNotes $Notes -Confirm:$Yes | Out-Null }
    "set" { Invoke-ReleaseFlow -SetVersion $Version -Publish -ReleaseNotes $Notes -Confirm:$Yes | Out-Null }
    "current" { Invoke-ReleaseFlow -Publish -ReleaseNotes $Notes -Confirm:$Yes | Out-Null }
    "package-patch" { Invoke-ReleaseFlow -Bump patch -Confirm:$Yes | Out-Null }
    "package" { Invoke-ReleaseFlow -Confirm:$Yes | Out-Null }
    "status" {
        $current = Get-ProjectVersion
        Write-Host "$current  $(Get-BumpedVersion $current 'patch')  $(Get-CurrentBranch)"
    }
    "notes" { Get-ChangelogNotes }
    default { throw "未知选项 $Action。可用 patch、minor、major、set、current、package-patch、package、status、notes。" }
}
