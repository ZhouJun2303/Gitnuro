package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IAddSubmoduleGitAction {
    suspend operator fun invoke(repositoryPath: String, name: String, path: String, uri: String): Either<Unit, GitError>
}