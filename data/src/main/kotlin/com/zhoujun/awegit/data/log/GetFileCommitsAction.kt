package com.zhoujun.awegit.data.log

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.JGitCommitMapper
import com.zhoujun.awegit.domain.interfaces.IGetFileCommitsAction
import javax.inject.Inject

class GetFileCommitsAction @Inject constructor(
    private val commitMapper: JGitCommitMapper,
    private val jgit: JGit,
) : IGetFileCommitsAction {
    override suspend fun invoke(
        repositoryPath: String,
        filePath: String
    ) = jgit.provide(repositoryPath) { git ->
        git.log()
            .addPath(filePath)
            .call()
            .toList()
            .map { commitMapper.toDomain(it) }
    }
}