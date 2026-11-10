# GitHub API calls from this network often drop at the TLS handshake.
# Each attempt is a fresh gh process with a fresh connection, so retrying
# is cheap and one timeout says nothing about the remote state.

function Invoke-GhOnce([string[]]$Arguments) {
    # Redirected native stderr would be a terminating error under "Stop".
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & gh @Arguments 2>&1
        $code = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previous
    }
    return [pscustomobject]@{
        Code = $code
        Text = (($output | Out-String).Trim())
    }
}

function Invoke-GhRetry {
    param(
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$What,
        [int]$Attempts = 6
    )
    $last = $null
    for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
        $last = Invoke-GhOnce $Arguments
        if ($last.Code -eq 0) { return $last.Text }
        if ($attempt -lt $Attempts) {
            $first = ($last.Text -split "`r?`n" | Where-Object { $_ } | Select-Object -First 1)
            Write-Host "$What 没有成功，正在重试 ($attempt)：$first"
            Start-Sleep -Seconds ([Math]::Min(3 * $attempt, 15))
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
        $last = Invoke-GhOnce @("release", "view", $Tag, "--repo", $Repo)
        if ($last.Code -eq 0) { return $true }
        if ($last.Text -match "release not found") { return $false }
        if ($attempt -lt 6) {
            Write-Host "查询 GitHub release $Tag 没有成功，正在重试 ($attempt)"
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
        $last = Invoke-GhOnce @("auth", "status")
        if ($last.Code -eq 0) { return }
        if ($last.Text -match "not logged|Failed to log in|Bad credentials|HTTP 401") { break }
        if ($attempt -lt 6) {
            Write-Host "检查 gh 登录没有成功，正在重试 ($attempt)"
            Start-Sleep -Seconds ([Math]::Min(3 * $attempt, 15))
        }
    }
    throw "gh 还没登录或连不上 GitHub：$($last.Text)`n没登录的话先运行 gh auth login"
}
