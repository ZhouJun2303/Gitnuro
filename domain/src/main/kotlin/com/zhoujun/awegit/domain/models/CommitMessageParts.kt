package com.zhoujun.awegit.domain.models

object CommitMessageParts {
    data class Parts(val summary: String, val description: String)

    fun split(message: String): Parts {
        val text = message.replace("\r\n", "\n").trimEnd()
        return Parts(text.substringBefore('\n').trim(), text.substringAfter('\n', "").trimStart('\n').trimEnd())
    }

    fun join(summary: String, description: String): String =
        if (description.isBlank()) summary.trim() else summary.trim() + "\n\n" + description.trimEnd()
}
