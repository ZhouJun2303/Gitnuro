package com.zhoujun.awegit.data.git.remotes

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IAddRemoteGitAction
import org.eclipse.jgit.transport.URIish
import javax.inject.Inject

class AddRemoteGitAction @Inject constructor(
    private val jgit: JGit,
) : IAddRemoteGitAction {
    override suspend operator fun invoke(
        repositoryPath: String,
        remoteName: String,
        fetchUri: String,
    ) = jgit.provide(repositoryPath) { git ->
        git
            .remoteAdd()
            .setName(remoteName)
            .setUri(URIish(fetchUri))
            .call()

        Unit
    }
}