package com.zhoujun.awegit.domain.repositories

import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import kotlinx.coroutines.flow.Flow

data class AiChatMessage(val role: String, val content: String)

data class AiChatRequest(
    val baseUrl: String,
    val apiKey: String,
    val model: String,
    val messages: List<AiChatMessage>,
    val temperature: Double?,
)

class AiException(message: String, val statusCode: Int? = null) : Exception(message)

interface AiRepository {
    suspend fun listModels(baseUrl: String, apiKey: String): Either<List<String>, AppError>

    /** Emits content chunks. Failures are thrown as [AiException]. */
    fun streamChat(request: AiChatRequest): Flow<String>
}
