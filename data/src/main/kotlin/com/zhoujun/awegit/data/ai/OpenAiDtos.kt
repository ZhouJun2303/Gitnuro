package com.zhoujun.awegit.data.ai

import kotlinx.serialization.Serializable

@Serializable
data class ChatMessageDto(val role: String, val content: String)

@Serializable
data class ChatCompletionRequestDto(
    val model: String,
    val messages: List<ChatMessageDto>,
    val stream: Boolean,
    val temperature: Double? = null,
)

@Serializable
data class ChatCompletionChunkDto(val choices: List<ChunkChoiceDto> = emptyList())

@Serializable
data class ChunkChoiceDto(val delta: ChunkDeltaDto? = null)

@Serializable
data class ChunkDeltaDto(val content: String? = null)

@Serializable
data class ChatCompletionResponseDto(val choices: List<ResponseChoiceDto> = emptyList())

@Serializable
data class ResponseChoiceDto(val message: ResponseMessageDto? = null)

@Serializable
data class ResponseMessageDto(val content: String? = null)

@Serializable
data class ModelsResponseDto(val data: List<ModelDto> = emptyList())

@Serializable
data class ModelDto(val id: String)
