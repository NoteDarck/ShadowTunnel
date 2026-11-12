# ShadowTunnel protocol

Frames use a 4-byte unsigned big-endian payload length followed by UTF-8 JSON. Maximum payload: 1 MiB.

Messages are tagged JSON objects: `auth`, `auth_ok`, `auth_error`, `heartbeat`, `heartbeat_ack`, `open`, `open_ok`, `open_error`, `data`, `close`, `open_udp`, `datagram`, `close_udp`.

The agent sends `auth` with a configured token and tunnel name. The gateway accepts only an exact token match, then both sides exchange heartbeats. `open` creates one local TCP stream. `data` carries bytes; `close` ends it. `open_udp` creates one authenticated UDP association. `datagram` carries one datagram up to 1200 bytes; `close_udp` ends it. IDs are scoped to one authenticated connection.

TLS is represented by configuration and a secure listener hook, but certificate/key loading is mandatory when enabled. No certificates are generated or embedded.
