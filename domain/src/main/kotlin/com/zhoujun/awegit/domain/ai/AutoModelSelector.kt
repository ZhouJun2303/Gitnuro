package com.zhoujun.awegit.domain.ai

object AutoModelSelector {
    private val excluded = listOf(
        "embed", "audio", "realtime", "tts", "whisper", "transcribe", "dall-e", "image", "moderation", "search",
    )

    fun pick(ids: List<String>): String? {
        val chat = ids.filter { id -> excluded.none { id.contains(it, ignoreCase = true) } }
        return chat.firstOrNull { it.contains("mini", ignoreCase = true) } ?: chat.firstOrNull()
    }
}
