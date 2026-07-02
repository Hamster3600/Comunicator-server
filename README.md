# Comunicator Server

Central server for the Comunicator chat system – handles registration, message routing, and user persistence.

---

## What it does

- Registers users and assigns permanent UUID-based hashes
- Routes messages between clients using a dynamic routing table
- Queues messages for offline users (`pending`) and delivers them on `sync`
- User listing, nick changes, hash regeneration – all over JSON/HTTP
- Stores all user data in the system keyring (survives restarts)
- Pending messages are RAM-only (lost on restart)

---

## Download

Get the latest binary from **[Releases](https://github.com/Hamster3600/Comunicator-server/releases)**.

```bash
chmod +x comunicator-server
./comunicator-server
```

---

## Build from source

```bash
cargo build --release
./target/release/comunicator-server
```

Or use the scripts:
```bash
chmod +x build.sh install.sh
./install.sh   # builds + copies to /usr/local/bin
```

---

## API Reference

**All requests:** `POST http://<server_ip>:4001/` with JSON body.

| Action | Request Body | Response |
|--------|-------------|----------|
| **Register** | `{"nick": "..."}` | hash (plain text) or `[ERROR] Nick is already taken.` |
| **Send message** | `{"target_hash":"...", "sender_hash":"...", "sender_nick":"...", "content":"..."}` | `[SERVER] Message forwarded...` or error |
| **List users** | `{"action": "list_users"}` | `{"users": [{"nick":"...", "hash":"..."}]}` |
| **Sync messages** | `{"action": "sync", "hash": "..."}` | `{"messages": [...]}` |
| **Change nick** | `{"action": "change_nick", "hash": "...", "new_nick": "..."}` | `[OK] Nick Changed.` or `[ERROR] Nick already taken.` |
| **Change hash** | `{"action": "change_hash", "old_hash": "..."}` | new hash (plain text) or `[ERROR] Invalid hash.` |

---

## Architecture

```
Client A ──POST msg──▶  Server  ──forward──▶  Client B (port 4001)
                            │
                            ├─ routing_table: HashMap<hash, ip:port>
                            ├─ pending:       HashMap<hash, Vec<MessagePacket>>
                            └─ keyring:       nick ↔ hash mappings (persistent)
```

- **Routing table** – maps hashes to `ip:port`. Updated every time a client sends a message. Ephemeral (RAM).
- **Pending queue** – holds messages for offline/unknown targets. Delivered on `sync`. Ephemeral (RAM).
- **Keyring (SQLite)** – stores `{nick}_hash`, `{nick}_nick`, and `{hash} → nick` reverse mapping. Persistent.

---

## Limitations

- Pending messages are lost on server restart
- No authentication beyond knowing a hash
- No online/offline presence tracking
- No rate limiting or spam protection
- LAN-only – no TLS, no encryption

---

## Dependencies

| Crate | Used for |
|-------|----------|
| `tiny_http` | HTTP server |
| `reqwest` (blocking) | Forwarding messages as HTTP client |
| `serde` / `serde_json` | JSON serialization |
| `keyring` | Persistent nick/hash storage (SQLite) |
| `uuid` | Generating unique hashes |

## Client side

**[Client](https://github.com/Hamster3600/Comunicator-client)**
---

## License

MIT – go nuts.
