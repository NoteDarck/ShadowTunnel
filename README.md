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

FASE 2 suporta Minecraft Java via TCP. FASE 3 adiciona Minecraft Bedrock via UDP. Use `SHADOW_MINECRAFT_TYPE=java` com destino padrão `127.0.0.1:25565`, ou `SHADOW_MINECRAFT_TYPE=bedrock` com destino padrão `127.0.0.1:19132`. O transporte correspondente é `tcp` ou `udp`, configurável por `SHADOW_TRANSPORT`. Para Bedrock, o Gateway publica UDP em `SHADOW_PUBLIC_UDP_BIND` (padrão `0.0.0.0:19132`). O destino do Agent aceita somente loopback. Compatibilidade TCP adicional para Geyser será configurada em etapa posterior. Teste externo: execute o servidor Minecraft local e conecte o cliente ao endereço público do Gateway.

Protocolo: `protocol/PROTOCOL.md`.

## Verificação

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```
