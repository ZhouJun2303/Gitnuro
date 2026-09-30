package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IStageByDirectoryGitAction {
    suspend operator fun invoke(repositoryPath: String, dir: String): Either<Unit, GitError>
}