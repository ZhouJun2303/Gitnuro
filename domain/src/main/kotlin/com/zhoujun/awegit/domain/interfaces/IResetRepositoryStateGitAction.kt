package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IResetRepositoryStateGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<Unit, GitError>
}