package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.ai.AutoModelSelector
import com.zhoujun.awegit.domain.ai.PromptTemplate
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.okOrNull
import com.zhoujun.awegit.domain.interfaces.IGetCommitDiffTextGitAction
import com.zhoujun.awegit.domain.interfaces.IGetRecentCommitMessagesGitAction
import com.zhoujun.awegit.domain.interfaces.IGetStagedDiffTextGitAction
import com.zhoujun.awegit.domain.repositories.AiChatMessage
import com.zhoujun.awegit.domain.repositories.AiChatRequest
import com.zhoujun.awegit.domain.repositories.AiException
import com.zhoujun.awegit.domain.repositories.AiRepository
import com.zhoujun.awegit.domain.repositories.DataState
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import com.zhoujun.awegit.domain.services.AppSettingsService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import javax.inject.Inject

sealed interface CommitMessageSource {
    data object Staged : CommitMessageSource
    data class ExistingCommit(val hash: String) : CommitMessageSource
}

private const val SYSTEM_PROMPT =
    "You are an expert software engineer who writes clear, conventional git commit messages. Reply with the commit message only."

class GenerateCommitMessageUseCase @Inject constructor(
    private val appSettingsService: AppSettingsService,
    private val aiRepository: AiRepository,
    private val repositoryDataRepository: RepositoryDataRepository,
    private val getStagedDiffTextGitAction: IGetStagedDiffTextGitAction,
    private val getCommitDiffTextGitAction: IGetCommitDiffTextGitAction,
    private val getRecentCommitMessagesGitAction: IGetRecentCommitMessagesGitAction,
) {
    private val autoModelCache = mutableMapOf<String, String>()

    operator fun invoke(source: CommitMessageSource): Flow<String> = flow {
        val settings = appSettingsService.aiSettings.first()
        val apiKey = settings.apiKey.ifBlank { System.getenv("OPENAI_API_KEY").orEmpty() }
        val baseUrl = settings.baseUrl.trimEnd('/')
        val repositoryPath = repositoryDataRepository.repositoryPath ?: throw AiException("No repository is open")

        val diff = when (source) {
            CommitMessageSource.Staged -> getStagedDiffTextGitAction(repositoryPath, settings.maxDiffChars)
            is CommitMessageSource.ExistingCommit -> getCommitDiffTextGitAction(repositoryPath, source.hash, settings.maxDiffChars)
        }.let { if (it is Either.Ok) it.value else throw AiException("Could not read the diff: $it") }
        if (diff.files.isEmpty()) throw AiException("There are no changes to describe")

        val recent = getRecentCommitMessagesGitAction(repositoryPath, 10).okOrNull().orEmpty()
        val branch = repositoryDataRepository.currentBranch.first { it !is DataState.Loading }.dataOrNull()?.simpleName ?: "HEAD"
        val prompt = PromptTemplate.render(
            settings.promptTemplate,
            mapOf(
                "diff" to diff.text,
                "files" to diff.files.joinToString("\n"),
                "branch" to branch,
                "recent_commits" to recent.joinToString("\n") { "- $it" },
                "language" to settings.language,
            ),
        )
        val model = settings.model.ifBlank {
            autoModelCache[baseUrl] ?: run {
                val ids = aiRepository.listModels(baseUrl, apiKey).okOrNull().orEmpty()
                AutoModelSelector.pick(ids)?.also { autoModelCache[baseUrl] = it }
                    ?: throw AiException("Could not pick a model automatically. Choose one in Settings > AI.")
            }
        }
        emitAll(
            aiRepository.streamChat(
                AiChatRequest(
                    baseUrl = baseUrl,
                    apiKey = apiKey,
                    model = model,
                    messages = listOf(AiChatMessage("system", SYSTEM_PROMPT), AiChatMessage("user", prompt)),
                    temperature = settings.temperature,
                )
            )
        )
    }.flowOn(Dispatchers.IO)
}
