package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.SignOffConfig

interface ISaveLocalRepositoryConfigGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        signOffConfig: SignOffConfig,
    ): Either<Unit, GitError>
}