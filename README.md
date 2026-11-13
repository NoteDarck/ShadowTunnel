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

## Plugin Paper

Compile o plugin em `plugin/` com Java 21 e copie `build/libs/ShadowTunnel-1.0.0.jar` para `plugins/`. Configure `plugins/ShadowTunnel/config.yml` com `api-url`, `api-token`, `tunnel.name`, `local-type` e `local-destination`. Comandos: `/shadowtunnel status`, `/shadowtunnel connect`, `/shadowtunnel disconnect` e `/shadowtunnel info`; exigem `shadowtunnel.admin`.

## Distribuição do Agent

Pré-requisitos: Rust via rustup, Cargo e ferramentas de compactação. Os scripts compilam apenas o alvo nativo de cada plataforma; cross-compilation não é prometida. O workflow usa runners Linux e Windows separados.

Linux x86_64:

```sh
./scripts/build-agent.sh
sudo install -Dm755 target/x86_64-unknown-linux-gnu/release/shadowtunnel-agent /usr/local/bin/shadowtunnel-agent
sudo install -d -m750 /etc/shadowtunnel
sudo install -m640 config/agent.env.example /etc/shadowtunnel/agent.env
sudoedit /etc/shadowtunnel/agent.env
sudo install -m644 packaging/systemd/shadowtunnel-agent.service.example /etc/systemd/system/shadowtunnel-agent.service
sudo systemctl daemon-reload
sudo systemctl enable --now shadowtunnel-agent
```

O script produz `dist/shadowtunnel-agent-linux-x86_64.tar.gz`. Para executar manualmente, carregue as variáveis de `config/agent.env.example` sem deixar o token em histórico e execute `./target/x86_64-unknown-linux-gnu/release/shadowtunnel-agent`.

Windows x86_64 (PowerShell):

```powershell
.\scripts\build-agent.ps1
Copy-Item config\agent.env.example agent.env
# Edite agent.env, substituindo o token; mantenha o arquivo privado.
.\target\x86_64-pc-windows-msvc\release\shadowtunnel-agent.exe
```

O script produz `dist\\shadowtunnel-agent-windows-x86_64.zip`; veja `packaging/windows/README.txt` para instalação persistente.

Variáveis do Agent: `SHADOW_TOKEN` (obrigatória), `SHADOW_GATEWAY`, `SHADOW_TUNNEL`, `SHADOW_MINECRAFT_TYPE`, `SHADOW_TRANSPORT` e `SHADOW_DESTINATION`. O destino aceita somente loopback. Use token longo e aleatório, permissões restritas no arquivo de ambiente e nunca registre tokens.

## TLS do canal Gateway-Agent

TLS é opcional e não altera o transporte público TCP/UDP. O padrão permanece plaintext. No Gateway, defina `SHADOW_TLS=1`, `SHADOW_TLS_CERT=/caminho/cert.pem` e `SHADOW_TLS_KEY=/caminho/key.pem`; ambos são PEM existentes e obrigatórios. No Agent, defina `SHADOW_TLS=1`, `SHADOW_TLS_CA=/caminho/ca.pem` e `SHADOW_TLS_SERVER_NAME=gateway.example`; a CA e o nome DNS são validados. Os dois lados devem usar TLS simultaneamente. Sem essas variáveis, a conexão Agent permanece TCP plaintext.

Não exponha a porta do Agent ou a API sem autenticação e firewall. Não inclua segredos em releases, logs ou commits.

## Verificação

```sh
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```
