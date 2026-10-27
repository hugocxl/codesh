# Mobile access

`codesh-server` can accept phone clients built on the
[`codesh-mobile`](../../crates/codesh-mobile/README.md) library. They connect
straight to the server on your computer, with no cloud service and no account,
and see the same projects and terminals as your desktop. Codesh does not
publish a phone app of its own; mobile access is off until you turn it on.

A phone connects in one of two ways:

- **Paired:** over your local network, a VPN such as Tailscale, or a public
  address. The connection is encrypted and the phone trusts only your server.
- **Over SSH:** the phone logs in like `codesh --host` does. No pairing or open
  port needed. See [Remote servers](remote-servers.md).

## Pair a phone

From the desktop app:

1. Open **Settings → Mobile** and turn on **Allow mobile devices**.
2. Under **Pair a phone**, choose **Show Pairing Code**.
3. Scan the code with the phone client.

From a shell, on any computer with `codesh`:

```bash
codesh mobile enable
codesh mobile pair
```

- A pairing code works once, for five minutes, and only while it is on screen.
- The code lists your computer's addresses. If phones reach it by another name,
  such as a cloud server's public name, put it first:
  `codesh mobile pair --address devbox.example.com`.
- Phones connect on port `7419` by default. Change it in **Settings → Mobile**
  or with `codesh mobile enable --port <port>`.

## Manage phones

| Task | Desktop | CLI |
| --- | --- | --- |
| See status and phones | **Settings → Mobile** | `codesh mobile` |
| Revoke a phone | **Revoke** next to it | `codesh mobile revoke <id>` |
| Turn access off | **Allow mobile devices** off | `codesh mobile disable` |

Revoking a phone, or turning access off, disconnects it at once.

## What a phone can do

A paired phone gets full terminal access: it can use projects, terminals, Git,
and files. It can't manage mobile access, change server settings, stop the
server, or run programs with `exec`.

## Troubleshooting

- On macOS, allow `codesh-server` if the firewall asks.
- On iOS, the client needs local network permission.
- The phone and computer must reach each other: same network, a VPN, or an
  address added with `--address`.
