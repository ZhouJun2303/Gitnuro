package com.zhoujun.awegit.data.git.stash

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.JGitCommitMapper
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.interfaces.ICreateSnapshotStashGitAction
import com.zhoujun.awegit.domain.models.Commit
import javax.inject.Inject

class CreateSnapshotStashGitAction @Inject constructor(
    private val commitMapper: JGitCommitMapper,
    private val jgit: JGit,
) : ICreateSnapshotStashGitAction {
    override suspend fun invoke(
        repositoryPath: String,
        message: String,
        includeUntracked: Boolean,
    ): Either<Commit?, GitError> = jgit.provide(repositoryPath) { git ->
        SnapshotStashCreateCommand(
            repository = git.repository,
            workingDirectoryMessage = message,
            includeUntracked = true
        )
            .call()
            ?.let { commitMapper.toDomain(it) }
    }
}