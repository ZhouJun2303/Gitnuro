package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IStashChangesGitAction {
    suspend operator fun invoke(repositoryPath: String, message: String?): Either<Unit, GitError>
}