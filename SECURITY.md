# Security and privacy notes

VXSStun is an early open-source client by VenoXiss / vpnxis, not a VPN provider.
It includes no VPN servers, account service, analytics or beta-key infrastructure.

- Provider credentials and subscription URLs are local: DPAPI CurrentUser on
  Windows; Android Keystore AES-GCM with backup disabled on Android.
- The WebView receives profile summaries, not outbound credentials or saved
  subscription URLs. Imported labels are rendered as text, never executable HTML.
- The WebView is restricted to bundled assets by navigation policy/CSP. Community
  links have a fixed native allowlist and open in the external browser.
- TLS certificate verification is required for HTTPS subscriptions and TLS
  profiles. Subscription redirects and URL credentials are rejected.
- Imports and stored data are size-limited. Subscription updates are transactional.
- Windows native runtimes are SHA-256 pinned. Xray is an owned child process;
  its TUN adapter is session-scoped. Other VPNs and firewall/proxy settings are not
  disabled by this client. TUN/elevation is always user-initiated.

Limitations: no independent security audit; no app-owned kill switch. Direct
network traffic can resume when the process/tunnel stops, including reconnects.
TUN readiness/HTTPS probes are not a comprehensive DNS/WebRTC/IPv6 leak test.
Windows bootstrap hostname resolution uses OS networking; Android uses a
protected bootstrap DNS socket. SOCKS/HTTP alone are not encrypted protocols.
The Android artifact uses a development key; Windows EXE has no Authenticode
publisher signature. These alpha artifacts are not production-certified.

When reporting a problem, send version, OS, protocol and sanitized reproduction
steps. Never post real subscription links, passwords, private keys or raw profile
stores in public GitHub issues or Telegram messages. There is no dedicated private
disclosure endpoint announced yet; arrange a private channel with the maintainer
before sharing sensitive details. Do not upload someone else's traffic captures.
