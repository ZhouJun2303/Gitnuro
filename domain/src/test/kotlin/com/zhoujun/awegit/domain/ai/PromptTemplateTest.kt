package com.zhoujun.awegit.domain.ai

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class PromptTemplateTest {
    @Test
    fun `replaces placeholders`() {
        val rendered = PromptTemplate.render(
            "Branch {{branch}}\n{{diff}}",
            mapOf("branch" to "main", "diff" to "diff-body"),
        )
        assertEquals("Branch main\ndiff-body", rendered)
    }

    @Test
    fun `blank template falls back and missing diff placeholder is appended`() {
        val rendered = PromptTemplate.render(" ", mapOf("diff" to "body", "language" to "English"))
        assertTrue(rendered.contains("Write a git commit message"))
        assertTrue(rendered.contains("body"))
    }

    @Test
    fun `template without diff placeholder appends the diff`() {
        val rendered = PromptTemplate.render("Only files", mapOf("diff" to "the-diff"))
        assertTrue(rendered.endsWith("the-diff"))
    }
}
