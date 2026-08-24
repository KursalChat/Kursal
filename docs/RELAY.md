# Running a Kursal Relay

A relay helps Kursal users reach each other. It acts as a libp2p relay for NAT traversal, a Kademlia DHT node for peer discovery and offline message mailboxes, and a bootstrap point for new peers joining the network. Relays never see message contents. Everything is end-to-end encrypted between peers, so a relay only ever forwards opaque ciphertext.

Relays are community-run. The more of them exist, the more resilient the network gets.

## Quickstart (Recommended)

We built a script that handles the whole setup for you.

```sh
curl -fsSL https://kursal.chat/relay.sh | bash
```

It detects your OS and lets you pick between Docker and a manual install.

<details>
<summary>Docker (manual)</summary>

Either pull the compose file (easier):

```sh
wget https://raw.githubusercontent.com/KursalChat/Kursal/main/docker/relay/compose.yml
docker compose up -d
```

Or run it directly:

```sh
docker run -d --name kursal-relay \
  --restart unless-stopped \
  -v "$PWD":/data \
  -p 4891:4891/tcp -p 4891:4891/udp \
  ghcr.io/kursalchat/relay:latest
```

First start writes a default `relay.toml` into the mounted folder. Edit it, set `announce_addr` to your public IP or DNS name, then restart the container (not required but recommended).

</details>

<details>
<summary>Manual (binary + systemd)</summary>

If you'd rather not run Docker, grab the binary directly and manage it with systemd.

Download the tarball for your architecture and extract it:

```sh
curl -fSL https://app.kursal.chat/kursal-relay-linux-x86_64.tar.gz -o kursal-relay.tar.gz
tar -xzf kursal-relay.tar.gz
sudo mv kursal-relay /usr/local/bin/kursal-relay
sudo chmod +x /usr/local/bin/kursal-relay
```

Use `kursal-relay-linux-aarch64.tar.gz` if you're on arm64.

Install the systemd unit and start the service:

```sh
wget https://raw.githubusercontent.com/KursalChat/Kursal/main/docker/relay/kursal-relay.service
sudo mv kursal-relay.service /etc/systemd/system/kursal-relay.service
sudo systemctl daemon-reload
sudo systemctl enable --now kursal-relay
```

First start writes a default `relay.toml` into `/var/lib/kursal-relay`. Edit `announce_addr` to your public IP or DNS name, then restart:

```sh
sudo systemctl restart kursal-relay
```

Check on it anytime with:

```sh
journalctl -u kursal-relay -f
```

</details>

## Your relay identity, do not lose it

`relay_identity.key` in `/data` (or `/var/lib/kursal-relay` for manual installs) **is** your relay's peer ID. Every client config and bootstrap list that references your relay embeds that peer ID in the multiaddr. If the key is lost, your relay comes back as a stranger and every reference to it breaks.

## Configuration (`relay.toml`)

| Key                                          | Meaning                                                                          |
| -------------------------------------------- | -------------------------------------------------------------------------------- |
| `listen_addr`                                | Bind address for the swarm, default `0.0.0.0:4891` (TCP + QUIC on the same port) |
| `announce_addr`                              | Public address advertised to the network (set this to your server's IP/DNS)      |
| `max_connections` / `max_connections_per_ip` | Connection limits                                                                |
| `bootstrap_peers`                            | Other relays to join the DHT through. It still uses defaults ones                |
| `log_level`, `log_file`                      | Logging; leave `log_file` unset for stdout                                       |
| `[health]`                                   | Local health endpoint, default `127.0.0.1:4892`                                  |

Open **4891 TCP and UDP** in your firewall. The health port (4892) can also optionally be exposed, which is recommended if you ask for your relay to be a bootstrap one. The health port exposes one HTTP /health path which returns the following:

```json
{
  "status": "ok",
  "peer_id": "some_peer_id",
  "uptime_secs": 123,
  "connections": 3
}
```

## Monitoring

- `curl http://127.0.0.1:4892/health` - peer ID, uptime, connection count.
- `kursal-relay --tui` - live terminal dashboard (connections, reservations,
  circuits, traffic sparklines, CPU/memory, event log). Interactive use only;
  run the service headless.

## Updating

If using a `compose.yml` file:

```sh
docker compose pull
docker compose up -d
```

Or:

```sh
docker pull ghcr.io/kursalchat/relay:latest
docker rm -f kursal-relay
docker run -d --name kursal-relay ... # same flags as above
```

## Building from source

```sh
cargo build -p kursal-cli --release   # target/release/kursal-relay
```

or reproduce the release artifacts (Docker required):

```sh
bin/build-relay.sh          # image + dist/kursal-relay-linux-x86_64.tar.gz
bin/build-relay.sh --push   # additionally push ghcr images
```

## Getting listed as a bootstrap relay

Stable, long-lived relays can be added to the default bootstrap list that
ships with the app. Open an issue with your relay's multiaddr
(`/dns4/relay.example.org/udp/4891/quic-v1/p2p/<peer-id>`) and how long you
intend to run it. We then may add it to [bootstrap.json](../kursal-core/src/network/bootstrap.json) and on our [status page](https://status.kursal.chat).
