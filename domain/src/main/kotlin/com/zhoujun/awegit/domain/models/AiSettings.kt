package com.zhoujun.awegit.domain.models

import kotlinx.serialization.Serializable

@Serializable
data class AiSettings(
    val enabled: Boolean = false,
    val baseUrl: String = DEFAULT_BASE_URL,
    val apiKey: String = "",
    val model: String = "",
    val language: String = "English",
    val maxDiffChars: Int = 12_000,
    val promptTemplate: String = DEFAULT_PROMPT_TEMPLATE,
    val temperature: Double? = null,
) {
    companion object {
        const val DEFAULT_BASE_URL = "https://api.openai.com/v1"
        val PLACEHOLDERS = listOf("diff", "files", "branch", "recent_commits", "language")
        val DEFAULT_PROMPT_TEMPLATE = """
            Write a git commit message for the staged changes below.
            Rules:
            - First line: imperative summary, at most 72 characters.
            - Then an empty line and an optional short body (bullet points) explaining what changed and why.
            - Write the message in {{language}}.
            - Output only the commit message, without code fences or explanations.

            Current branch: {{branch}}

            Recent commits (match their style):
            {{recent_commits}}

            Changed files:
            {{files}}

            Diff:
            {{diff}}
        """.trimIndent()
    }
}
