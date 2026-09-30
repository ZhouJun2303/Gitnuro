package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.GitFlowBranchType
import com.zhoujun.awegit.domain.models.GitFlowConfig

interface IGitFlowGitAction {
    suspend fun load(repositoryPath: String): Either<GitFlowConfig, GitError>
    suspend fun save(repositoryPath: String, config: GitFlowConfig): Either<Unit, GitError>
    suspend fun init(repositoryPath: String, config: GitFlowConfig): Either<Unit, GitError>
    suspend fun start(repositoryPath: String, type: GitFlowBranchType, name: String): Either<Unit, GitError>
    suspend fun finish(
        repositoryPath: String,
        type: GitFlowBranchType,
        name: String,
        deleteBranch: Boolean,
        tagMessage: String,
    ): Either<Unit, GitError>
}
