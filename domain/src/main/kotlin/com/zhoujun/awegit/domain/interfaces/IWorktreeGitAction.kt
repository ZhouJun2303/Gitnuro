package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.WorktreeListResult

interface IWorktreeGitAction {
    suspend fun list(repositoryPath: String): Either<WorktreeListResult, GitError>
    suspend fun remove(repositoryPath: String, path: String): Either<Unit, GitError>
}
