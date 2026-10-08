param([string]$Binary="onekey.exe", [ValidateSet("chrome","edge")][string]$Browser="chrome", [string]$ExtensionId="")
$ErrorActionPreference="Stop"
if (-not $ExtensionId) {
  Write-Host "Load the extension/ directory at ${Browser}://extensions, enable Developer mode, then copy its ID."
  $ExtensionId=Read-Host "Extension ID"
}
& $Binary browser install --browser $Browser --extension-id $ExtensionId
if ($LASTEXITCODE -ne 0) {throw "OneKey native host installation failed"}
Write-Host "Open the OneKey extension -> Manage websites and accounts. Existing bindings are preserved."
Write-Host "After upgrading the CLI, re-run this installer to refresh the Windows host executable."
Write-Host "For unpacked extension upgrades, replace extension/ at the SAME location and click Reload."
