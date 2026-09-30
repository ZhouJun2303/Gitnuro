package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.common.extensions.TAG
import com.zhoujun.awegit.common.printError
import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.onErr
import com.zhoujun.awegit.domain.interfaces.IPersistCommitMessageGitAction
import javax.inject.Inject

class PersistCommitMessageUseCase @Inject constructor(
    private val persistCommitMessageGitAction: IPersistCommitMessageGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend operator fun invoke(message: String?) {
        val messageToPersist = message?.ifBlank { null }
        useCaseExecutor.execute { repositoryPath ->
            val result = persistCommitMessageGitAction(repositoryPath, messageToPersist)
                .onErr {
                    printError(TAG, "Failed to persist commit message: $it")
                }

            result
        }
    }
}
