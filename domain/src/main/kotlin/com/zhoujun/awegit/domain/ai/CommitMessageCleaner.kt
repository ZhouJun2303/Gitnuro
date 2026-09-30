package com.zhoujun.awegit.domain.ai

object CommitMessageCleaner {
    fun clean(raw: String): String {
        var text = raw.replace(Regex("(?s)<think>.*?</think>"), "").trim()
        if (text.startsWith("```")) text = text.substringAfter('\n', "").substringBeforeLast("```").trim()
        return text
    }
}
