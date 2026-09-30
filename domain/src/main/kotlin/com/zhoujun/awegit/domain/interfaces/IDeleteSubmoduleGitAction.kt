package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IDeleteSubmoduleGitAction {
    suspend operator fun invoke(repositoryPath: String, path: String): Either<Unit, GitError>
}