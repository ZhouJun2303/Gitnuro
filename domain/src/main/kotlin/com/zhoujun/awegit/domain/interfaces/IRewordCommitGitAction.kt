package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IRewordCommitGitAction {
    suspend operator fun invoke(repositoryPath: String, commitHash: String, newMessage: String): Either<Unit, GitError>
}
