package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.models.PersistedCommitMessage

interface IGetPersistedCommitMessagesGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<PersistedCommitMessage, AppError>
}