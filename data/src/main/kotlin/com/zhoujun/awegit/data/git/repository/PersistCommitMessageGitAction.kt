package com.zhoujun.awegit.data.git.repository

import com.zhoujun.awegit.data.extensions.isMerging
import com.zhoujun.awegit.data.extensions.isReverting
import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IPersistCommitMessageGitAction
import org.eclipse.jgit.lib.RepositoryState
import javax.inject.Inject

class PersistCommitMessageGitAction @Inject constructor(
    private val jgit: JGit,
) : IPersistCommitMessageGitAction {
    override suspend operator fun invoke(repositoryPath: String, message: String?) =
        jgit.provide(repositoryPath) { git ->
            val state = git.repository.repositoryState
            if (state.isMerging || state.isRebasing || state.isReverting) {
                git.repository.writeMergeCommitMsg(message)
            } else if (state == RepositoryState.SAFE) {
                git.repository.writeCommitEditMsg(message)
            }
        }
}
