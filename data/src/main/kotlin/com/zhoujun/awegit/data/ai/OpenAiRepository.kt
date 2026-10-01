package com.zhoujun.awegit.data.ai

import com.zhoujun.awegit.domain.errors.AiRequestError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.repositories.AiChatRequest
import com.zhoujun.awegit.domain.repositories.AiException
import com.zhoujun.awegit.domain.repositories.AiRepository
import io.ktor.client.HttpClient
import io.ktor.client.request.accept
import io.ktor.client.request.get
import io.ktor.client.request.header
import io.ktor.client.request.preparePost
import io.ktor.client.request.setBody
import io.ktor.client.statement.bodyAsChannel
import io.ktor.client.statement.bodyAsText
import io.ktor.http.ContentType
import io.ktor.http.HttpHeaders
import io.ktor.http.contentType
import io.ktor.http.isSuccess
import io.ktor.utils.io.readUTF8Line
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.channelFlow
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import javax.inject.Inject
import javax.inject.Named

class OpenAiRepository @Inject constructor(
    @Named(AI_HTTP_CLIENT) private val httpClient: HttpClient,
) : AiRepository {
    private val json = Json { ignoreUnknownKeys = true; explicitNulls = false }

    override suspend fun listModels(baseUrl: String, apiKey: String): Either<List<String>, com.zhoujun.awegit.domain.errors.AppError> = try {
        val response = httpClient.get("${baseUrl.trimEnd('/')}/models") { authorize(apiKey) }
        if (!response.status.isSuccess()) {
            Either.Err(AiRequestError("HTTP ${response.status.value}: ${response.bodyAsText().take(300)}"))
        } else {
            Either.Ok(json.decodeFromString<ModelsResponseDto>(response.bodyAsText()).data.map { it.id }.sorted())
        }
    } catch (e: Exception) {
        Either.Err(AiRequestError(e.message ?: e::class.simpleName.orEmpty()))
    }

    override fun streamChat(request: AiChatRequest): Flow<String> = channelFlow {
        val body = json.encodeToString(
            ChatCompletionRequestDto(
                model = request.model,
                messages = request.messages.map { ChatMessageDto(it.role, it.content) },
                stream = true,
                temperature = request.temperature,
            )
        )
        httpClient.preparePost("${request.baseUrl.trimEnd('/')}/chat/completions") {
            authorize(request.apiKey)
            contentType(ContentType.Application.Json)
            accept(ContentType.Text.EventStream)
            setBody(body)
        }.execute { response ->
            if (!response.status.isSuccess()) {
                throw AiException("HTTP ${response.status.value}: ${response.bodyAsText().take(500)}", response.status.value)
            }
            if (response.contentType()?.match(ContentType.Text.EventStream) != true) {
                val full = json.decodeFromString<ChatCompletionResponseDto>(response.bodyAsText())
                send(full.choices.firstOrNull()?.message?.content.orEmpty())
                return@execute
            }
            val channel = response.bodyAsChannel()
            while (!channel.isClosedForRead) {
                val line = channel.readUTF8Line() ?: break
                if (!line.startsWith("data:")) continue
                val data = line.removePrefix("data:").trim()
                if (data == "[DONE]") break
                if (data.isEmpty()) continue
                val delta = json.decodeFromString<ChatCompletionChunkDto>(data).choices.firstOrNull()?.delta?.content
                if (!delta.isNullOrEmpty()) send(delta)
            }
        }
    }

    private fun io.ktor.client.request.HttpRequestBuilder.authorize(apiKey: String) {
        if (apiKey.isNotBlank()) header(HttpHeaders.Authorization, "Bearer $apiKey")
    }
}
