package com.zhoujun.awegit.domain.ai

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class AutoModelSelectorTest {
    @Test
    fun `prefers a mini chat model and skips non-chat ids`() {
        assertEquals(
            "gpt-4o-mini",
            AutoModelSelector.pick(listOf("text-embedding-3-large", "gpt-4o", "gpt-4o-mini", "whisper-1")),
        )
    }

    @Test
    fun `falls back to the first chat model`() {
        assertEquals("gpt-4o", AutoModelSelector.pick(listOf("dall-e-3", "gpt-4o")))
    }

    @Test
    fun `returns null when nothing looks like a chat model`() {
        assertNull(AutoModelSelector.pick(listOf("whisper-1", "text-embedding-3-small")))
    }
}
