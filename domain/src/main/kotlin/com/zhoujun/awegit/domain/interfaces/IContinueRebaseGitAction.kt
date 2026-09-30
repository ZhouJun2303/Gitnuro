package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IContinueRebaseGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<Unit, GitError>
}