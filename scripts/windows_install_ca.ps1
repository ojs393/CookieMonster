# Trust Cookie Monster Local CA in the current-user Root store.
$ErrorActionPreference = "Stop"
$candidates = @(
  Join-Path $env:LOCALAPPDATA "cookie-monster\local-ca\cookie-monster-ca.cer",
  Join-Path $env:LOCALAPPDATA "com.CookieMonster.cookie-monster\local-ca\cookie-monster-ca.cer"
)
$cert = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cert) {
  Write-Error "CA cert not found. Start the native host once, then re-run."
}
certutil -user -addstore Root $cert
Write-Host "Trusted: $cert"
