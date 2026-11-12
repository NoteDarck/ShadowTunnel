# ShadowTunnel Windows agent

Prerequisites: Windows x86_64, PowerShell 5+, and a Rust toolchain with the `x86_64-pc-windows-msvc` target. Run `rustup target add x86_64-pc-windows-msvc` if needed.

Build from the repository root:

```powershell
.\scripts\build-agent.ps1
```

The artifact is written to `dist\shadowtunnel-agent-windows-x86_64.zip`. Extract it to a protected directory, create `agent.env` from `config\agent.env.example`, set a real token, then run:

```powershell
Get-Content .\agent.env | ForEach-Object { if ($_ -match '^([^#][^=]*)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process') } }
.\shadowtunnel-agent.exe
```

Use Task Scheduler or a service manager for persistence. Do not expose the token, `agent.env`, or process environment to untrusted users. The current agent connects with TCP and does not provide TLS itself; use a TLS-protected gateway endpoint or a trusted private network until gateway TLS is configured.
