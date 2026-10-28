# Toggle WinINET user proxy to Cookie Monster (127.0.0.1:8877).
param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("on", "off")]
  [string]$Action
)
$ErrorActionPreference = "Stop"
$key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings"
if ($Action -eq "on") {
  Set-ItemProperty -Path $key -Name ProxyEnable -Value 1
  Set-ItemProperty -Path $key -Name ProxyServer -Value "127.0.0.1:8877"
  Write-Host "WinINET proxy → 127.0.0.1:8877"
} else {
  Set-ItemProperty -Path $key -Name ProxyEnable -Value 0
  Write-Host "WinINET proxy disabled"
}
Add-Type -Namespace WinI -Name N -MemberDefinition @"
[DllImport("wininet.dll")] public static extern bool InternetSetOption(IntPtr h, int o, IntPtr b, int l);
"@
[WinI.N]::InternetSetOption([IntPtr]::Zero, 39, [IntPtr]::Zero, 0) | Out-Null
[WinI.N]::InternetSetOption([IntPtr]::Zero, 37, [IntPtr]::Zero, 0) | Out-Null
