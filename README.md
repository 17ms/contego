<div align="center" style="text-align:center">
    <picture>
        <source media="(prefers-color-scheme: dark)" srcset="https://github.com/17ms/contego/blob/master/docs/contego-dark.png">
        <img src="https://github.com/17ms/contego/blob/master/docs/contego-light.png" width="800">
    </picture>
</div>

<p align="left">
<a href="https://github.com/17ms/contego/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/17ms/contego/ci.yml?branch=main"></a>
<a href="https://github.com/17ms/contego/tags"><img src="https://img.shields.io/github/v/tag/17ms/contego"></a>
<a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/github/license/17ms/contego"></a>
</p>

## Cryptographic specifications

The initial key exchange is performed with elliptic-curve Diffie-Hellman. General data exchange is encrypted with AES-GCM. During regular communication payloads are Base64 encoded before being encrypted to prevent delimiter conflicts. SHA-256 hashes of files are compared to ensure data integrity.

## Cellular networks

Most cellular ISP's tend to block port forwarding on CGNAT level, which makes it impossible to create inbound connections to such network without a VPN. Luckily many consumer VPNs and self-hosted solutions make port forwarding a trivial task. This is the main reason why the client must fetch information about the public IP from an external service (https://ipinfo.io/ip for IPv4 and https://ipv6.icanhazip.com for IPv6). 

## Usage

Work in progress. Will be completed when the current release is finished.

```shell
cargo build --release
./target/release/contego
```
