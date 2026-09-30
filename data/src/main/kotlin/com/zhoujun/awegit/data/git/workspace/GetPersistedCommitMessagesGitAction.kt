package com.zhoujun.awegit.data.git.workspace

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.interfaces.IGetPersistedCommitMessagesGitAction
import com.zhoujun.awegit.domain.models.PersistedCommitMessage
import javax.inject.Inject

class GetPersistedCommitMessagesGitAction @Inject constructor(
    private val jgit: JGit,
) : IGetPersistedCommitMessagesGitAction {
    override suspend fun invoke(repositoryPath: String): Either<PersistedCommitMessage, AppError> {
        return jgit.provide(repositoryPath) { git ->
            val commitMessage = git.repository.readCommitEditMsg()
            val mergeMessage = git.repository.readMergeCommitMsg()
            val squashMessage = git.repository.readSquashCommitMsg()

            PersistedCommitMessage(commitMessage, mergeMessage, squashMessage)
        }
    }
}