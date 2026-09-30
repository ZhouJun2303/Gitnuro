package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Commit

interface IGetCommitFromHashGitAction {
    suspend operator fun invoke(repositoryPath: String, commitHash: String): Either<Commit?, GitError>
}