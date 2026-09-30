package com.zhoujun.awegit.ui.dialogs

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.ai.CommitMessageCleaner
import com.zhoujun.awegit.domain.errors.GenericError
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.CommitMessageParts
import com.zhoujun.awegit.domain.models.TaskType
import com.zhoujun.awegit.domain.repositories.FailureSeverity
import com.zhoujun.awegit.domain.repositories.RepositoryStateRepository
import com.zhoujun.awegit.domain.services.AppSettingsService
import com.zhoujun.awegit.domain.usecases.CommitMessageSource
import com.zhoujun.awegit.domain.usecases.GenerateCommitMessageUseCase
import com.zhoujun.awegit.domain.usecases.RewordCommitUseCase
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlin.coroutines.cancellation.CancellationException

class RewordCommitDialogViewModel @AssistedInject constructor(
    private val rewordCommitUseCase: RewordCommitUseCase,
    private val generateCommitMessageUseCase: GenerateCommitMessageUseCase,
    private val repositoryStateRepository: RepositoryStateRepository,
    appSettingsService: AppSettingsService,
    @Assisted val commit: Commit,
) : TabViewModel() {
    @AssistedFactory
    interface Factory {
        fun create(commit: Commit): RewordCommitDialogViewModel
    }

    private val initial = CommitMessageParts.split(commit.message)
    val summary = MutableStateFlow(TextFieldValue(initial.summary))
    val description = MutableStateFlow(TextFieldValue(initial.description))
    val isGenerating = MutableStateFlow(false)
    val isAiEnabled = appSettingsService.aiSettings.map { it.enabled }
    private var generateJob: Job? = null

    fun save() = rewordCommitUseCase(commit.hash, CommitMessageParts.join(summary.value.text, description.value.text))

    fun generate() {
        generateJob?.cancel()
        generateJob = viewModelScope.launch {
            isGenerating.value = true
            var text = ""
            try {
                generateCommitMessageUseCase(CommitMessageSource.ExistingCommit(commit.hash)).collect { chunk ->
                    text += chunk
                    applyGenerated(text)
                }
                applyGenerated(CommitMessageCleaner.clean(text))
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                repositoryStateRepository.addCompletedTaskFailed(
                    TaskType.GenerateCommitMessage,
                    GenericError(e.message.orEmpty(), e),
                    FailureSeverity.HIGH,
                )
            } finally {
                isGenerating.value = false
            }
        }
    }

    fun cancelGenerate() {
        generateJob?.cancel()
    }

    private fun applyGenerated(raw: String) {
        val parts = CommitMessageParts.split(raw)
        summary.value = TextFieldValue(parts.summary, selection = TextRange(parts.summary.length))
        description.value = TextFieldValue(parts.description, selection = TextRange(parts.description.length))
    }
}
