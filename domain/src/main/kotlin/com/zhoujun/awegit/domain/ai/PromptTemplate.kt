package com.zhoujun.awegit.domain.ai

import com.zhoujun.awegit.domain.models.AiSettings

object PromptTemplate {
    fun render(template: String, values: Map<String, String>): String {
        val source = template.ifBlank { AiSettings.DEFAULT_PROMPT_TEMPLATE }
        var result = source
        for ((key, value) in values) result = result.replace("{{$key}}", value)
        if (!source.contains("{{diff}}")) result += "\n\nDiff:\n" + values["diff"].orEmpty()
        return result
    }
}
