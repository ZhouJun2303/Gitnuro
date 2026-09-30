package com.zhoujun.awegit.di.modules

import com.zhoujun.awegit.data.ai.AI_HTTP_CLIENT
import dagger.Provides
import io.ktor.client.*
import io.ktor.client.engine.cio.*
import io.ktor.client.plugins.HttpTimeout
import io.ktor.client.plugins.logging.*
import java.security.cert.X509Certificate
import javax.inject.Named
import javax.net.ssl.X509TrustManager

@dagger.Module
class NetworkModule {
    @Provides
    fun provideKtorHttpClient(): HttpClient {
        val httpClient = HttpClient(CIO) {
            install(Logging) {
                logger = Logger.DEFAULT
                level = LogLevel.NONE
            }
            engine {
                https {
                    trustManager = object : X509TrustManager {
                        override fun checkClientTrusted(p0: Array<out X509Certificate>?, p1: String?) {}

                        override fun checkServerTrusted(p0: Array<out X509Certificate>?, p1: String?) {}

                        override fun getAcceptedIssuers(): Array<X509Certificate>? = null
                    }
                }
            }
        }

        return httpClient
    }

    @Provides
    @Named(AI_HTTP_CLIENT)
    fun provideAiHttpClient(): HttpClient = HttpClient(CIO) {
        install(HttpTimeout) {
            connectTimeoutMillis = 15_000
            socketTimeoutMillis = 120_000
            requestTimeoutMillis = 300_000
        }
        engine { requestTimeout = 0 }
    }
}