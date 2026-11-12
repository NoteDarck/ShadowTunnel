# ShadowTunnel

Workspace Rust com `gateway`, `agent` e `protocol`.

## Execução

```sh
export SHADOW_TOKEN='token-secreto'
cargo run -p shadowtunnel-gateway
export SHADOW_GATEWAY=127.0.0.1:9000
export SHADOW_DESTINATION=127.0.0.1:25565
export SHADOW_MINECRAFT_TYPE=java
export SHADOW_TUNNEL=default
cargo run -p shadowtunnel-agent
```

Gateway: `SHADOW_PUBLIC_BIND` (padrão `0.0.0.0:8080`), `SHADOW_AGENT_BIND` (padrão `0.0.0.0:9000`) e `SHADOW_TUNNEL` (padrão `default`). O listener público usa somente esse túnel; conexões são rejeitadas quando o agente não está conectado. TLS preparado: defina `SHADOW_TLS=1`, `SHADOW_TLS_CERT` e `SHADOW_TLS_KEY` com arquivos existentes; nenhum certificado é criado.

FASE 2 suporta Minecraft Java via TCP. `SHADOW_MINECRAFT_TYPE` aceita somente `java` por enquanto. `SHADOW_DESTINATION` continua configurável, com padrão `127.0.0.1:25565`. Teste externo: execute um servidor Minecraft Java local nessa porta e conecte o cliente ao endereço público do gateway. O fluxo interno pode ser validado com um servidor TCP local/echo, sem serviços externos.

Protocolo: `protocol/PROTOCOL.md`.

## Verificação

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```
