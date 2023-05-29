<div align="center" style="text-align:center">
    <picture>
        <source media="(prefers-color-scheme: dark)" srcset="https://github.com/17ms/contego/blob/master/.github/docs/contego-dark.png">
        <img src="https://github.com/17ms/contego/blob/master/.github/docs/contego-light.png" width="800">
    </picture>
</div>

<p align="center">
<a href="https://github.com/17ms/contego/actions/workflows/cargo-checkmate.yaml"><img src="https://img.shields.io/github/actions/workflow/status/17ms/contego/cargo-checkmate.yaml?branch=master"></a>
<a href="https://github.com/17ms/contego/tags"><img src="https://img.shields.io/github/v/tag/17ms/contego"></a>
<a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/github/license/17ms/contego"></a>
</p>

## Cryptographic specifications

The initial key exchange is performed with elliptic-curve Diffie-Hellman. General data exchange is encrypted with AES-GCM. During regular communication payloads are Base64 encoded before being encrypted to prevent delimiter conflicts. SHA-256 hashes of files are compared to ensure data integrity.

## Cellular networks

Most cellular ISP's tend to block port forwarding on CGNAT level, which makes it impossible to create inbound connections to such network without a VPN. Luckily many consumer VPNs and self-hosted solutions make port forwarding a trivial task. This is the main reason why the client must fetch information about the public IP from an external service (https://ipinfo.io/ip for IPv4 and https://ipv6.icanhazip.com for IPv6). 

## Usage

Check [releases](https://github.com/17ms/contego/releases) for an up-to-date executables or build from source with `cargo build --release`.

### Server

```
Usage: contego host [OPTIONS] --key <KEY> <--source <SOURCE>|--files <FILES>...>

Options:
  -k, --key <KEY>              Access key
  -s, --source <SOURCE>        Path to a source file (alternative to --files)
  -f, --files <FILES>...       Paths to shareable files (alternative to --source)
  -p, --port <PORT>            Host port [default: 8080]
  -6, --ipv6                   IPv6 instead of IPv4
  -c, --chunksize <CHUNKSIZE>  Transmit chunksize in bytes [default: 8192]
  -l, --local                  Host locally
  -h, --help                   Print help

```

### Client

```
Usage: contego connect --addr <ADDR> --out <OUT> --key <KEY>

Options:
  -a, --addr <ADDR>  IP address of the instance
  -o, --out <OUT>    Path to an output folder
  -k, --key <KEY>    Access key
  -h, --help         Print help
```
