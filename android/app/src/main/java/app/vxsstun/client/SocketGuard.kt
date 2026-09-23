package app.vxsstun.client

import libXray.DialerController
import libXray.LibXray

// libXray installs a process-wide dialer hook. Register a stable bridge once,
// rather than retaining a new VpnService instance on every connection.
internal object SocketGuard: DialerController {
    @Volatile private var owner: TunnelService? = null
    private var registered = false

    @Synchronized fun claim(service: TunnelService) {
        check(owner == null || owner === service) { "Предыдущее подключение ещё завершается" }
        if (!registered) { LibXray.registerDialerController(this); registered = true }
        owner = service
    }

    @Synchronized fun release(service: TunnelService, close: () -> Unit) {
        if (owner === service) {
            try { close() } finally { owner = null }
        }
    }

    override fun protectFd(fd: Long): Boolean = owner?.protect(fd.toInt()) ?: false
}
