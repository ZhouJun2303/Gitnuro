package com.zhoujun.awegit.data.ai

import com.zhoujun.awegit.domain.repositories.AiChatMessage
import com.zhoujun.awegit.domain.repositories.AiChatRequest
import com.zhoujun.awegit.domain.repositories.AiException
import io.ktor.client.HttpClient
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.http.ContentType
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.runBlocking
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows

class OpenAiRepositoryTest {
    private val request = AiChatRequest(
        baseUrl = "https://example.test/v1",
        apiKey = "secret",
        model = "gpt-test",
        messages = listOf(AiChatMessage("user", "hi")),
        temperature = null,
    )

    @Test
    fun `sse chunks are concatenated until done`() = runBlocking {
        var sawAuth = false
        val repo = repository { httpRequest ->
            sawAuth = httpRequest.headers[HttpHeaders.Authorization] == "Bearer secret"
            respond(
                content = """
                    data: {"choices":[{"delta":{"content":"Hello"}}]}

                    data: {"choices":[{"delta":{"content":" world"}}]}

                    data: [DONE]

                """.trimIndent(),
                status = HttpStatusCode.OK,
                headers = headersOf(HttpHeaders.ContentType, ContentType.Text.EventStream.toString()),
            )
        }
        assertEquals("Hello world", repo.streamChat(request).toList().joinToString(""))
        assertTrue(sawAuth)
    }

    @Test
    fun `json responses are returned as a single chunk`() = runBlocking {
        val repo = repository {
            respond(
                content = """{"choices":[{"message":{"content":"full text"}}]}""",
                status = HttpStatusCode.OK,
                headers = headersOf(HttpHeaders.ContentType, ContentType.Application.Json.toString()),
            )
        }
        assertEquals(listOf("full text"), repo.streamChat(request).toList())
    }

    @Test
    fun `unauthorized responses throw with the status code`() = runBlocking {
        val repo = repository {
            respond("nope", HttpStatusCode.Unauthorized)
        }
        val error = assertThrows<AiException> {
            repo.streamChat(request).toList()
        }
        assertEquals(401, error.statusCode)
    }

    @Test
    fun `blank api key omits the authorization header`() = runBlocking {
        var authorization: String? = "present"
        val repo = repository { httpRequest ->
            authorization = httpRequest.headers[HttpHeaders.Authorization]
            respond(
                content = """{"choices":[{"message":{"content":"ok"}}]}""",
                status = HttpStatusCode.OK,
                headers = headersOf(HttpHeaders.ContentType, ContentType.Application.Json.toString()),
            )
        }
        repo.streamChat(request.copy(apiKey = "")).toList()
        assertNull(authorization)
    }

    private fun repository(handler: io.ktor.client.engine.mock.MockRequestHandler): OpenAiRepository {
        val client = HttpClient(MockEngine) { engine { addHandler(handler) } }
        return OpenAiRepository(client)
    }
}
