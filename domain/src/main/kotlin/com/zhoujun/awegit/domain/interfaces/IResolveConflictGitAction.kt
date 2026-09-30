package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.conflicts.ConflictSides
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IResolveConflictGitAction {
    suspend fun read(repositoryPath: String, path: String): Either<ConflictSides, GitError>
    suspend fun useStage(repositoryPath: String, path: String, stage: Int): Either<Unit, GitError>
    suspend fun markResolved(repositoryPath: String, path: String): Either<Unit, GitError>
}
