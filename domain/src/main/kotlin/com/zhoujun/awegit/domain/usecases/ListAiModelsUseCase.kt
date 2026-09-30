package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.repositories.AiRepository
import com.zhoujun.awegit.domain.services.AppSettingsService
import kotlinx.coroutines.flow.first
import javax.inject.Inject

class ListAiModelsUseCase @Inject constructor(
    private val appSettingsService: AppSettingsService,
    private val aiRepository: AiRepository,
) {
    suspend operator fun invoke(): Either<List<String>, AppError> {
        val settings = appSettingsService.aiSettings.first()
        val apiKey = settings.apiKey.ifBlank { System.getenv("OPENAI_API_KEY").orEmpty() }
        return aiRepository.listModels(settings.baseUrl, apiKey)
    }
}
