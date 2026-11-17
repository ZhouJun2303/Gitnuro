# Offline regression tests. All Git writes and simulated uploads stay in .lab.
$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot "gh-retry.ps1")

function Assert-True($Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Invoke-TestGit([string]$Directory, [string[]]$Arguments) {
    $output = & git -C $Directory @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Test git failed: $($Arguments -join ' ')" }
    return ($output | Out-String).Trim()
}

$mockGh = @'
function Start-Sleep { param([int]$Seconds) }
function Invoke-GhOnce([string[]]$Arguments, [string]$What) {
    Write-PublishLog $What
    $releaseFile = Join-Path $root ".git/mock-release"
    $assetsFile = Join-Path $root ".git/mock-assets"
    $eventsFile = Join-Path $root ".git/mock-events"
    $text = ""
    $code = 0
    switch ($Arguments[0] + " " + $Arguments[1]) {
        "auth status" { }
        "release view" {
            if (-not (Test-Path $releaseFile)) { $code = 1; $text = "release not found" }
            elseif ($Arguments -contains "isDraft") {
                $text = if ([IO.File]::ReadAllText($releaseFile) -eq "draft") { "true" } else { "false" }
            }
            elseif ($Arguments -contains "--json" -and (Test-Path $assetsFile)) {
                $text = [IO.File]::ReadAllText($assetsFile)
            }
        }
        "release create" {
            if ($Arguments -notcontains "--draft") { throw "An incomplete release must remain a draft" }
            if (@($Arguments | Where-Object { $_ -match '\.(exe|msi)$' }).Count) {
                throw "Release creation must be separate from installer uploads"
            }
            [IO.File]::AppendAllText($eventsFile, "create`n")
            [IO.File]::WriteAllText($releaseFile, "draft")
            if (Test-Path (Join-Path $root ".git/mock-create-timeout")) {
                $code = 1; $text = "TLS timeout after creating release"
            }
        }
        "release upload" {
            $name = [IO.Path]::GetFileName($Arguments[3])
            [IO.File]::AppendAllText($eventsFile, "upload $name`n")
            if ($name -like "*.msi" -and (Test-Path (Join-Path $root ".git/mock-fail-msi"))) {
                $code = 1; $text = "simulated upload failure"
            } else {
                [IO.File]::AppendAllText($assetsFile, "$name`n")
            }
        }
        "release edit" {
            if ($Arguments -notcontains "--draft=false") { throw "Unexpected release edit" }
            if (@(Get-Content -LiteralPath $assetsFile).Count -ne 2) { throw "Release published before both installers were uploaded" }
            [IO.File]::WriteAllText($releaseFile, "public")
            [IO.File]::AppendAllText($eventsFile, "publish`n")
        }
        default { throw "Unexpected GitHub command: $($Arguments -join ' ')" }
    }
    [pscustomobject]@{ Code = $code; Text = $text }
}
'@

$mockPackage = @'
$fixtureRoot = Split-Path -Parent $PSScriptRoot
[IO.File]::AppendAllText((Join-Path $fixtureRoot ".git/mock-builds"), "build`n")
$fixtureVersion = (Get-Content -Raw (Join-Path $fixtureRoot "package.json") | ConvertFrom-Json).version
foreach ($kind in @("nsis", "msi")) {
    $directory = Join-Path $fixtureRoot "target/release/bundle/$kind"
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    $name = if ($kind -eq "nsis") { "AweGit_${fixtureVersion}_x64-setup.exe" } else { "AweGit_${fixtureVersion}_x64_en-US.msi" }
    [IO.File]::WriteAllText((Join-Path $directory $name), "offline installer fixture")
}
$global:LASTEXITCODE = 0
'@

function New-ReleaseFixture([string]$Name) {
    $directory = Join-Path $testBase $Name
    $work = Join-Path $directory "work"
    $remote = Join-Path $directory "remote.git"
    New-Item -ItemType Directory -Path $work -Force | Out-Null
    Invoke-TestGit $directory @("init", "--bare", "--quiet", $remote) | Out-Null
    Invoke-TestGit $directory @("init", "--quiet", "--initial-branch=feat/dev", $work) | Out-Null
    foreach ($file in @(
        ".gitignore", "package.json", "README.md", "Cargo.lock", "latest.json",
        "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "crates/git/Cargo.toml",
        "src/lib/shell/state.svelte.ts", "scripts/check-versions.mjs",
        "scripts/publish-menu.ps1", "scripts/publish-windows.ps1", "scripts/gh-retry.ps1"
    )) {
        $destination = Join-Path $work $file
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $projectRoot $file) -Destination $destination
    }
    # Leave the real retry and push helpers intact; replace only the network
    # boundary and expensive build with deterministic local fixtures.
    [IO.File]::AppendAllText((Join-Path $work "scripts/gh-retry.ps1"), "`n$mockGh", $utf8)
    [IO.File]::WriteAllText((Join-Path $work "scripts/package-windows.ps1"), $mockPackage, $utf8)
    Invoke-TestGit $work @("add", ".") | Out-Null
    Invoke-TestGit $work @("commit", "--quiet", "-m", "test: seed") | Out-Null
    Invoke-TestGit $work @("branch", "release") | Out-Null
    Invoke-TestGit $work @("remote", "add", "origin", $remote) | Out-Null
    Invoke-TestGit $work @("push", "--quiet", "origin", "feat/dev", "release") | Out-Null
    return $work
}

function Assert-Published([string]$Directory) {
    Assert-True ([IO.File]::ReadAllText((Join-Path $Directory ".git/mock-release")) -eq "public") "Completed release was left as a draft"
    Assert-True ((Invoke-TestGit $Directory @("branch", "--show-current")) -eq "feat/dev") "Did not return to feat/dev"
    Assert-True ((Invoke-TestGit $Directory @("rev-list", "--count", "origin/feat/dev..feat/dev")) -eq "0") "Development branch still has unpushed commits"
    $manifest = Get-Content -Raw -LiteralPath (Join-Path $Directory "latest.json") | ConvertFrom-Json
    $tagCommit = Invoke-TestGit $Directory @("rev-parse", "v$($manifest.appVersion)^{}")
    $releaseCommit = Invoke-TestGit $Directory @("rev-parse", "origin/release")
    Assert-True ($tagCommit -eq $releaseCommit) "Release tag and remote manifest commit differ"
    Assert-True (-not (Invoke-TestGit $Directory @("status", "--porcelain"))) "Publish left a dirty worktree"
}

$utf8 = New-Object System.Text.UTF8Encoding $false
$testBase = Join-Path $projectRoot (".lab/publish-test-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $testBase -Force | Out-Null
$savedEnvironment = @{}
foreach ($name in @("GIT_CONFIG_NOSYSTEM", "GIT_CONFIG_GLOBAL", "XDG_CONFIG_HOME", "GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "AWE_APP_CODE")) {
    $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, "Process")
}
try {
    # Isolate test identity and configuration without changing any gitconfig.
    $env:GIT_CONFIG_NOSYSTEM = "1"
    $env:XDG_CONFIG_HOME = $testBase
    $env:GIT_CONFIG_GLOBAL = Join-Path $testBase "empty-gitconfig"
    [IO.File]::WriteAllText($env:GIT_CONFIG_GLOBAL, "")
    $env:GIT_AUTHOR_NAME = $env:GIT_COMMITTER_NAME = "AweGit publish test"
    $env:GIT_AUTHOR_EMAIL = $env:GIT_COMMITTER_EMAIL = "publish-test@example.invalid"

    & {
        foreach ($file in @("gh-retry.ps1", "publish-menu.ps1", "publish-windows.ps1", "publish.test.ps1")) {
            $path = Join-Path $PSScriptRoot $file
            $bytes = [IO.File]::ReadAllBytes($path)
            Assert-True ([BitConverter]::ToString($bytes, 0, 3) -eq "EF-BB-BF") "PowerShell 5.1 needs a UTF-8 BOM for $file on ANSI Windows"
            $reader = [IO.StreamReader]::new($path, [Text.Encoding]::GetEncoding(936), $true)
            try { $source = $reader.ReadToEnd() } finally { $reader.Dispose() }
            $tokens = $null; $errors = $null
            [Management.Automation.Language.Parser]::ParseInput($source, [ref]$tokens, [ref]$errors) | Out-Null
            Assert-True (-not $errors) "Script does not parse on code page 936: $file"
        }
        $values = @("中文 空格", 'embedded"quote', 'C:\path with spaces\', '', 'slash\"quote', '$()`; &|%')
        $program = 'process.stdout.write(JSON.stringify(process.argv.slice(1)) + "\n"); process.stderr.write("pipe-error".repeat(10000)); setTimeout(() => process.exit(7), 2200)'
        $records = @(Invoke-PublishProcess -Command node -Arguments (@("-e", $program, "--") + $values) -What "heartbeat fixture" -HeartbeatSeconds 1 6>&1)
        $result = @($records | Where-Object { $_ -is [pscustomobject] -and $_.PSObject.Properties["Code"] })
        Assert-True ($result.Count -eq 1 -and $result[0].Code -eq 7) "Native exit code or pipeline result was lost"
        $decoded = ConvertFrom-Json -InputObject (($result[0].Text -split "`n")[0])
        Assert-True ($decoded.Count -eq $values.Count) "Windows native argument quoting changed the argument count"
        for ($i = 0; $i -lt $values.Count; $i++) {
            Assert-True ($decoded[$i] -ceq $values[$i]) "Windows native argument quoting changed argument $i"
        }
        Assert-True ($result[0].Text.Contains("pipe-error")) "Native stderr was lost"
        Assert-True (-not $result[0].Stdout.Contains("pipe-error") -and $result[0].Stderr.StartsWith("pipe-error")) "Native stderr contaminated stdout data"
        Assert-True ((($records | Out-String) -match '仍在执行')) "Silent command emitted no heartbeat"
        Write-Host "PASS: ANSI Windows encoding, heartbeat, native arguments, concurrent pipes and exit code"
    }

    & {
        $script:attempts = 0
        function Start-Sleep { param([int]$Seconds) }
        function Invoke-GhOnce([string[]]$Arguments, [string]$What) {
            $script:attempts++
            if ($script:attempts -eq 1) { return [pscustomobject]@{ Code = 1; Text = "TLS timeout" } }
            return [pscustomobject]@{ Code = 0; Text = "asset.exe" }
        }
        $records = @(Invoke-GhRetry @("release", "view") "retry fixture" 6>&1)
        Assert-True ($script:attempts -eq 2) "Transient request was not retried"
        Assert-True ($records[-1] -eq "asset.exe") "Logs contaminated the API return value"
        Assert-True (($records | Out-String) -match '3 秒后重试') "Retry delay was not logged"
        function Invoke-GhOnce { [pscustomobject]@{ Code = 1; Text = "TLS timeout" } }
        $failure = $null
        try { Test-GitHubRelease "v0.0.0" "offline/fixture" | Out-Null } catch { $failure = $_ }
        Assert-True ($null -ne $failure) "An unreachable API was mistaken for a missing release"
        Write-Host "PASS: retry logs preserve API data and reject an unreachable release"
    }

    & {
        $fixture = New-ReleaseFixture "complete"
        . (Join-Path $fixture "scripts/publish-menu.ps1")
        # A timeout after server-side creation must continue with that release.
        [IO.File]::WriteAllText((Join-Path $fixture ".git/mock-create-timeout"), "timeout")
        $records = @(Invoke-ReleaseFlow -Bump patch -Publish -ReleaseNotes "line one`nline two" -Confirm 6>&1)
        Assert-Published $fixture
        $events = @(Get-Content -LiteralPath (Join-Path $fixture ".git/mock-events"))
        Assert-True (@($events | Where-Object { $_ -eq "create" }).Count -eq 1) "Timed-out creation was duplicated"
        Assert-True (@($events | Where-Object { $_ -like "upload *" }).Count -eq 2) "Both installers were not uploaded individually"
        $logs = $records | Out-String
        Assert-True ($logs -match '上传附件 1/2' -and $logs -match '上传附件 2/2') "Per-installer progress was not logged"
        Write-Host "PASS: full publish pushes the version commit and recovers a creation timeout"
    }

    & {
        $fixture = New-ReleaseFixture "resume"
        . (Join-Path $fixture "scripts/publish-menu.ps1")
        $before = Invoke-TestGit $fixture @("rev-parse", "origin/release")
        $flag = Join-Path $fixture ".git/mock-fail-msi"
        [IO.File]::WriteAllText($flag, "fail")
        $failure = $null
        try { Invoke-ReleaseFlow -Bump patch -Publish -ReleaseNotes "offline test" -Confirm 6>&1 | Out-Null } catch { $failure = $_ }
        Assert-True ($null -ne $failure) "Upload failure was reported as success"
        Assert-True ([IO.File]::ReadAllText((Join-Path $fixture ".git/mock-release")) -eq "draft") "Upload failure exposed an incomplete public release"
        Assert-True ((Invoke-TestGit $fixture @("rev-parse", "origin/release")) -eq $before) "Manifest was pushed before all installers were uploaded"
        $manifestCommit = Invoke-TestGit $fixture @("rev-parse", "release")
        Assert-True ($manifestCommit -ne $before) "Failed upload discarded the manifest needed to resume"
        Remove-Item -LiteralPath $flag -Force
        Invoke-ReleaseFlow -Publish -ReleaseNotes "offline test" -Confirm 6>&1 | Out-Null
        Assert-Published $fixture
        Assert-True ((Invoke-TestGit $fixture @("rev-parse", "release")) -eq $manifestCommit) "Resume rewrote the tagged manifest commit"
        $events = @(Get-Content -LiteralPath (Join-Path $fixture ".git/mock-events"))
        Assert-True (@($events | Where-Object { $_ -like "upload *.exe" }).Count -eq 1) "Resume uploaded the completed installer again"
        Assert-True (@(Get-Content -LiteralPath (Join-Path $fixture ".git/mock-builds")).Count -eq 1) "Resume rebuilt the installers"
        Write-Host "PASS: upload failure preserves release and resumes only missing installers"
    }

    & {
        $fixture = New-ReleaseFixture "rejected-push"
        . (Join-Path $fixture "scripts/publish-menu.ps1")
        $before = Invoke-TestGit $fixture @("rev-parse", "release")
        $tree = Invoke-TestGit $fixture @("rev-parse", "HEAD^{tree}")
        $remoteCommit = Invoke-TestGit $fixture @("commit-tree", $tree, "-p", $before, "-m", "test: remote diverged")
        $remote = Join-Path (Split-Path -Parent $fixture) "remote.git"
        # Transfer the object without advancing any local tracking branch.
        Invoke-TestGit $fixture @("push", "--quiet", "origin", "${remoteCommit}:refs/heads/feat/dev") | Out-Null
        $failure = $null
        try { Invoke-ReleaseFlow -Bump patch -Publish -ReleaseNotes "offline test" -Confirm 6>&1 | Out-Null } catch { $failure = $_ }
        Assert-True ($null -ne $failure) "Non-fast-forward push was reported as success"
        Assert-True ((Invoke-TestGit $fixture @("branch", "--show-current")) -eq "feat/dev") "Rejected push switched branches"
        Assert-True ((Invoke-TestGit $fixture @("rev-parse", "release")) -eq $before) "Rejected development push changed release"
        Assert-True (-not (Test-Path (Join-Path $fixture ".git/mock-builds"))) "Rejected push still built installers"
        Assert-True ((Invoke-TestGit $remote @("rev-parse", "feat/dev")) -eq $remoteCommit) "Rejected push rewrote the remote branch"
        Write-Host "PASS: rejected development push stops before release changes or packaging"
    }

    & {
        $fixture = New-ReleaseFixture "package-only"
        . (Join-Path $fixture "scripts/publish-menu.ps1")
        $before = Invoke-TestGit $fixture @("rev-parse", "origin/feat/dev")
        Invoke-ReleaseFlow -Bump patch -Confirm 6>&1 | Out-Null
        Assert-True ((Invoke-TestGit $fixture @("rev-parse", "origin/feat/dev")) -eq $before) "Package-only mode pushed a branch"
        Assert-True ((Invoke-TestGit $fixture @("rev-list", "--count", "origin/feat/dev..feat/dev")) -eq "1") "Package-only mode did not keep its local version commit"
        Write-Host "PASS: package-only mode keeps its version commit local"
    }
    Write-Host "All 6 publish regression tests passed."
}
finally {
    foreach ($name in $savedEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], "Process")
    }
    # Delete only this run's unique directory, after checking the resolved path.
    $resolvedBase = (Resolve-Path -LiteralPath $testBase).Path
    $labRoot = [IO.Path]::GetFullPath((Join-Path $projectRoot ".lab")) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolvedBase.StartsWith($labRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove a fixture outside .lab: $resolvedBase"
    }
    Remove-Item -LiteralPath $resolvedBase -Recurse -Force
}
