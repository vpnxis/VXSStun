package app.vxsstun.client

object CommunityLinks {
    fun url(destination:String):String = when(destination) {
        "telegram" -> "https://t.me/VenoXiss"
        "github" -> "https://github.com/vpnxis"
        else -> error("Неизвестная ссылка команды")
    }
}
