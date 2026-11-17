# GitHub API calls from this network often drop at the TLS handshake.
# Each attempt is a fresh gh process with a fresh connection, so retrying
# is cheap and one timeout says nothing about the remote state.

function Write-PublishLog([string]$Message) {
    Write-Host ("[{0:HH:mm:ss}] {1}" -f (Get-Date), $Message)
}

# Windows PowerShell 5.1 has no ProcessStartInfo.ArgumentList. Quote each
# argument for the Windows native command line, including trailing slashes.
function ConvertTo-PublishArgument([string]$Value) {
    '"' + ([regex]::Replace(([regex]::Replace($Value, '(\\*)"', '$1$1\"')), '(\\+)$', '$1$1')) + '"'
}

function Invoke-PublishProcess {
    param(
        [string]$Command,
        [string[]]$Arguments,
        [string]$What,
        [ValidateRange(1, 3600)][int]$HeartbeatSeconds = 10,
        [switch]$ShowOutput
    )
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = (Get-Command $Command -CommandType Application -ErrorAction Stop).Source
    $start.Arguments = ($Arguments | ForEach-Object { ConvertTo-PublishArgument $_ }) -join ' '
    $start.WorkingDirectory = (Get-Location).ProviderPath
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.StandardOutputEncoding = New-Object System.Text.UTF8Encoding $false
    $start.StandardErrorEncoding = New-Object System.Text.UTF8Encoding $false
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $start
    $started = $false
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    Write-PublishLog "$What，开始"
    try {
        $process.Start() | Out-Null
        $started = $true
        # Drain both pipes without blocking. Git output also streams to the
        # console, including prompts that do not end with a newline.
        $streams = @(
            @{ Reader = $process.StandardOutput; Buffer = (New-Object char[] 4096); Text = (New-Object Text.StringBuilder) },
            @{ Reader = $process.StandardError; Buffer = (New-Object char[] 4096); Text = (New-Object Text.StringBuilder) }
        )
        foreach ($stream in $streams) {
            $stream.Task = $stream.Reader.ReadAsync($stream.Buffer, 0, $stream.Buffer.Length)
        }
        $nextLog = $HeartbeatSeconds
        do {
            foreach ($stream in $streams) {
                while ($stream.Task -and $stream.Task.IsCompleted) {
                    $length = $stream.Task.GetAwaiter().GetResult()
                    if ($length -eq 0) { $stream.Task = $null; break }
                    $chunk = [string]::new($stream.Buffer, 0, $length)
                    $stream.Text.Append($chunk) | Out-Null
                    if ($ShowOutput) { Write-Host $chunk -NoNewline }
                    $stream.Task = $stream.Reader.ReadAsync($stream.Buffer, 0, $stream.Buffer.Length)
                }
            }
            if ($watch.Elapsed.TotalSeconds -ge $nextLog) {
                Write-PublishLog ("$What，仍在执行，已等待 {0:N0} 秒" -f $watch.Elapsed.TotalSeconds)
                $nextLog = $watch.Elapsed.TotalSeconds + $HeartbeatSeconds
            }
            $waiting = $streams[0].Task -or $streams[1].Task -or -not $process.HasExited
            if ($waiting) { [System.Threading.Thread]::Sleep(100) }
        } while ($waiting)
        if ($ShowOutput) { Write-Host "" }
        $code = $process.ExitCode
        $state = if ($code -eq 0) { "完成" } else { "退出码 $code" }
        Write-PublishLog ("$What，$state，耗时 {0:N1} 秒" -f $watch.Elapsed.TotalSeconds)
        return [pscustomobject]@{
            Code = $code
            Stdout = $streams[0].Text.ToString().Trim()
            Stderr = $streams[1].Text.ToString().Trim()
            Text = (($streams | ForEach-Object { $_.Text.ToString() } | Where-Object { $_ }) -join "`n").Trim()
        }
    }
    finally {
        if ($started -and -not $process.HasExited) {
            $process.Kill()
        }
        $process.Dispose()
    }
}

function Invoke-GhOnce([string[]]$Arguments, [string]$What = "GitHub 请求") {
    Invoke-PublishProcess -Command gh -Arguments $Arguments -What $What
}

function Push-PublishRef([string]$Repository, [string]$Ref, [string]$What) {
    $last = $null
    foreach ($attempt in 1..3) {
        # This repository has no LFS files; avoid its redundant locks API call.
        $last = Invoke-PublishProcess -Command git -Arguments @(
            "-C", $Repository, "-c", "lfs.locksverify=false", "push", "--progress", "origin", $Ref
        ) -What "$What（第 $attempt/3 次）" -ShowOutput
        if ($last.Code -eq 0) { return }
        if ($attempt -lt 3) {
            Write-PublishLog "$What 没有成功，$($attempt * 5) 秒后重试"
            Start-Sleep -Seconds (5 * $attempt)
        }
    }
    throw "$What 失败：$($last.Text)"
}

function Invoke-GhRetry {
    param(
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$What,
        [int]$Attempts = 6
    )
    $last = $null
    for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
        $last = Invoke-GhOnce $Arguments "$What（第 $attempt/$Attempts 次）"
        if ($last.Code -eq 0) { return $last.Text }
        if ($attempt -lt $Attempts) {
            $first = ($last.Text -split "`r?`n" | Where-Object { $_ } | Select-Object -First 1)
            $delay = [Math]::Min(3 * $attempt, 15)
            Write-PublishLog "$What 没有成功，$delay 秒后重试：$first"
            Start-Sleep -Seconds $delay
        }
    }
    throw "$What 失败：$($last.Text)"
}

# $false only when GitHub itself answered that the release does not exist.
# An unreachable API is retried and then thrown: treating it as "missing"
# would repackage and try to create a release that may already be there.
function Test-GitHubRelease([string]$Tag, [string]$Repo) {
    $last = $null
    for ($attempt = 1; $attempt -le 6; $attempt++) {
        $last = Invoke-GhOnce @("release", "view", $Tag, "--repo", $Repo) "查询 GitHub release $Tag（第 $attempt/6 次）"
        if ($last.Code -eq 0) {
            Write-PublishLog "GitHub release $Tag 已存在"
            return $true
        }
        if ($last.Text -match "release not found") {
            Write-PublishLog "GitHub release $Tag 尚未创建"
            return $false
        }
        if ($attempt -lt 6) {
            Write-PublishLog "查询 GitHub release $Tag 没有成功，正在重试 ($attempt)"
            Start-Sleep -Seconds ([Math]::Min(3 * $attempt, 15))
        }
    }
    throw "无法确认 GitHub 上是否已有 $Tag：$($last.Text)"
}

# gh auth status verifies the token against the API, so a network timeout
# looks like broken credentials. Retry those; stop at real login errors.
function Assert-GhAuthenticated {
    $last = $null
    for ($attempt = 1; $attempt -le 6; $attempt++) {
        $last = Invoke-GhOnce @("auth", "status") "检查 GitHub 登录（第 $attempt/6 次）"
        if ($last.Code -eq 0) { return }
        if ($last.Text -match "not logged|Failed to log in|Bad credentials|HTTP 401") { break }
        if ($attempt -lt 6) {
            Write-PublishLog "检查 gh 登录没有成功，正在重试 ($attempt)"
            Start-Sleep -Seconds ([Math]::Min(3 * $attempt, 15))
        }
    }
    throw "gh 还没登录或连不上 GitHub：$($last.Text)`n没登录的话先运行 gh auth login"
}
