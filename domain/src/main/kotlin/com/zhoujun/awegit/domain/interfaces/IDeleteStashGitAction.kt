package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Commit

interface IDeleteStashGitAction {
    suspend operator fun invoke(repositoryPath: String, stashInfo: Commit): Either<Unit, GitError>
}