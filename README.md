# ShadowTunnel

Workspace Rust com `gateway`, `agent` e `protocol`.

## Execução

```sh
export SHADOW_TOKEN='token-secreto'
cargo run -p shadowtunnel-gateway
export SHADOW_GATEWAY=127.0.0.1:9000
export SHADOW_DESTINATION=127.0.0.1:3000
cargo run -p shadowtunnel-agent
```

Gateway: `SHADOW_PUBLIC_BIND` (padrão `0.0.0.0:8080`) e `SHADOW_AGENT_BIND` (padrão `0.0.0.0:9000`). TLS preparado: defina `SHADOW_TLS=1`, `SHADOW_TLS_CERT` e `SHADOW_TLS_KEY` com arquivos existentes; nenhum certificado é criado.

Protocolo: `protocol/PROTOCOL.md`.

## Verificação

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```
