package com.zhoujun.awegit.data.git.remotes

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.interfaces.IDeleteRemoteGitAction
import javax.inject.Inject

class DeleteRemoteGitAction @Inject constructor(
    private val jgit: JGit,
) : IDeleteRemoteGitAction {
    override suspend operator fun invoke(repositoryPath: String, remoteName: String): Either<Unit, GitError> {
        return jgit.provide(repositoryPath) { git ->
            git
                .remoteRemove()
                .setRemoteName(remoteName)
                .call()
        }
    }
}