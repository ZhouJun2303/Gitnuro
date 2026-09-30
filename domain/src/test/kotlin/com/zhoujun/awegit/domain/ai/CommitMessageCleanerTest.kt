package com.zhoujun.awegit.domain.ai

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class CommitMessageCleanerTest {
    @Test
    fun `strips think blocks and wrapping fences`() {
        val raw = """
            <think>reasoning</think>
            ```
            Fix login
            ```
        """.trimIndent()
        assertEquals("Fix login", CommitMessageCleaner.clean(raw))
    }

    @Test
    fun `leaves a plain message unchanged`() {
        assertEquals("Add tests", CommitMessageCleaner.clean("  Add tests  "))
    }
}
