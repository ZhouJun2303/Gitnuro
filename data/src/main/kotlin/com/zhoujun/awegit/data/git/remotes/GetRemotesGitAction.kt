package com.zhoujun.awegit.data.git.remotes

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.RemoteConfigToRemoteMapper
import com.zhoujun.awegit.domain.interfaces.IGetRemotesGitAction
import javax.inject.Inject

class GetRemotesGitAction @Inject constructor(
    private val remoteMapper: RemoteConfigToRemoteMapper,
    private val jgit: JGit,
) : IGetRemotesGitAction {
    override suspend operator fun invoke(repositoryPath: String) = jgit.provide(repositoryPath) { git ->
        git
            .remoteList()
            .call()
            .map { remoteConfig -> remoteMapper.toDomain(remoteConfig) }
    }
}