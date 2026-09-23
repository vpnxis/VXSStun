package app.vxsstun.client
import org.junit.Assert.*
import org.junit.Test

class CommunityLinksTest {
    @Test fun opensOnlyTeamLinks() {
        assertEquals("https://t.me/VenoXiss",CommunityLinks.url("telegram"))
        assertEquals("https://github.com/vpnxis",CommunityLinks.url("github"))
        for(value in listOf("file:///", "intent://", "https://evil.invalid", "")) {
            assertTrue(runCatching { CommunityLinks.url(value) }.isFailure)
        }
    }
}
