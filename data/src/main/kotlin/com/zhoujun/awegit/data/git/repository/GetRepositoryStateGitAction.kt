package com.zhoujun.awegit.data.git.repository

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.RepositoryStateMapper
import com.zhoujun.awegit.domain.interfaces.IGetRepositoryStateGitAction
import javax.inject.Inject

class GetRepositoryStateGitAction @Inject constructor(
    private val jgit: JGit,
    private val repositoryStateMapper: RepositoryStateMapper,
) : IGetRepositoryStateGitAction {
    override suspend operator fun invoke(repositoryPath: String) = jgit.provide(repositoryPath) { git ->
        repositoryStateMapper.toDomain(git.repository.repositoryState)
    }
}