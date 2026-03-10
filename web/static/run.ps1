$ErrorActionPreference = "Stop"

$Repo = "Freeskier/steply"
$Version = if ($env:STEPLY_VERSION) { $env:STEPLY_VERSION } else { "latest" }

function Test-IsWindows {
  if ($env:OS -eq "Windows_NT") {
    return $true
  }

  return [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
  )
}

if (-not (Test-IsWindows)) {
  throw "run.ps1 is intended for Windows. Use /run on Unix systems."
}

$Asset = "steply-x86_64-pc-windows-msvc.zip"
$BaseUrl = if ($Version -eq "latest") {
  "https://github.com/$Repo/releases/latest/download"
} else {
  "https://github.com/$Repo/releases/download/$Version"
}

$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("steply-run-" + [guid]::NewGuid().ToString("N"))
$Archive = Join-Path $TmpDir $Asset
$ExtractDir = Join-Path $TmpDir "extract"

New-Item -ItemType Directory -Force -Path $TmpDir | Out-Null
New-Item -ItemType Directory -Force -Path $ExtractDir | Out-Null

try {
  Invoke-WebRequest -Uri "$BaseUrl/$Asset" -OutFile $Archive
  Expand-Archive -LiteralPath $Archive -DestinationPath $ExtractDir -Force
  & (Join-Path $ExtractDir "steply.exe") @args
} finally {
  Remove-Item -LiteralPath $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
