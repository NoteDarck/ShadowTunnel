$ErrorActionPreference = 'Stop'

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo is required (install Rust via rustup).'
}
if (-not (Get-Command Compress-Archive -ErrorAction SilentlyContinue)) {
    throw 'Compress-Archive is required (PowerShell 5+).'
}

$Root = Split-Path -Parent $PSScriptRoot
$Target = 'x86_64-pc-windows-msvc'
$OutputDir = Join-Path $Root 'dist'
$Archive = Join-Path $OutputDir 'shadowtunnel-agent-windows-x86_64.zip'
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
cargo build --release -p shadowtunnel-agent --target $Target
Compress-Archive -Force -Path (Join-Path $Root "target/$Target/release/shadowtunnel-agent.exe") -DestinationPath $Archive
Write-Output "Artifact: $Archive"
