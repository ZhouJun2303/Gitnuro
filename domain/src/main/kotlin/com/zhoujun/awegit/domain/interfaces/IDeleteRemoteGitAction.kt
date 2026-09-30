package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IDeleteRemoteGitAction {
    suspend operator fun invoke(repositoryPath: String, remoteName: String): Either<Unit, GitError>
}