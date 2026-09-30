package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IAddRemoteGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        remoteName: String,
        fetchUri: String,
    ): Either<Unit, GitError>
}