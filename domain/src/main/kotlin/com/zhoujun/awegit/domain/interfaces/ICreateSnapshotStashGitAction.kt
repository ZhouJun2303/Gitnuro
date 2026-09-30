package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Commit

interface ICreateSnapshotStashGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        message: String,
        includeUntracked: Boolean,
    ): Either<Commit?, GitError>
}