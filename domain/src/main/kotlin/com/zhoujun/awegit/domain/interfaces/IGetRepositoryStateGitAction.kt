package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.RepositoryState

interface IGetRepositoryStateGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<RepositoryState, GitError>
}