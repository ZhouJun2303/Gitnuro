# Report whether the WebView2 runtime is installed.
# The Windows bundle uses embedBootstrapper: the installer can download the
# runtime, and this repository does not ship an offline WebView2 installer.

$ErrorActionPreference = "Stop"
$guid = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
$keys = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$guid",
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$guid",
    "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$guid"
)
foreach ($key in $keys) {
    if (-not (Test-Path $key)) { continue }
    $version = (Get-ItemProperty -Path $key -ErrorAction SilentlyContinue).pv
    if ($version) {
        Write-Host "WebView2 $version ($key)"
        exit 0
    }
}
Write-Host "WebView2 runtime was not found. The installer bootstrapper can download it when the machine is online."
exit 1
