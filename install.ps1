# Install the OneKey CLI on Windows:
#   irm https://raw.githubusercontent.com/panp1/onekey-cli/main/install.ps1 | iex
# $env:ONEKEY_VERSION selects a release (default: latest); $env:ONEKEY_INSTALL_DIR the folder
# (default: %LOCALAPPDATA%\Programs\onekey), which is added to the user PATH.
$ErrorActionPreference = 'Stop'
$repo = 'panp1/onekey-cli'
$installDir = if ($env:ONEKEY_INSTALL_DIR) { $env:ONEKEY_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\onekey' }
$version = if ($env:ONEKEY_VERSION) { $env:ONEKEY_VERSION } else {
  (Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest").tag_name
}
$archive = "onekey-cli_${version}_windows_amd64.zip"
$base = "https://github.com/$repo/releases/download/$version"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  Write-Host "Downloading OneKey CLI $version (windows/amd64)..."
  Invoke-WebRequest "$base/$archive" -OutFile (Join-Path $tmp $archive)
  Invoke-WebRequest "$base/checksums.txt" -OutFile (Join-Path $tmp 'checksums.txt')
  $line = Get-Content (Join-Path $tmp 'checksums.txt') | Where-Object { $_ -match " $([regex]::Escape($archive))$" }
  if (-not $line) { throw "$archive is not listed in checksums.txt" }
  $expected = ($line -split ' ')[0]
  $actual = (Get-FileHash (Join-Path $tmp $archive) -Algorithm SHA256).Hash.ToLower()
  if ($expected -ne $actual) { throw "checksum mismatch for $archive" }
  Expand-Archive (Join-Path $tmp $archive) -DestinationPath $tmp -Force
  New-Item -ItemType Directory -Path $installDir -Force | Out-Null
  Copy-Item (Join-Path $tmp 'onekey.exe') (Join-Path $installDir 'onekey.exe') -Force
} finally {
  Remove-Item $tmp -Recurse -Force
}
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($userPath -split ';') -notcontains $installDir) {
  [Environment]::SetEnvironmentVariable('Path', "$userPath;$installDir", 'User')
  Write-Host "Added $installDir to your user PATH; open a new terminal."
}
& (Join-Path $installDir 'onekey.exe') --version
Write-Host 'Next: onekey config'
