package com.zhoujun.awegit.data.git.log

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IRewordCommitGitAction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.lib.RepositoryState
import javax.inject.Inject

class RewordCommitGitAction @Inject constructor(
    private val jgit: JGit,
) : IRewordCommitGitAction {
    override suspend fun invoke(repositoryPath: String, commitHash: String, newMessage: String) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            val repo = git.repository
            if (repo.repositoryState != RepositoryState.SAFE) {
                throw IllegalStateException("Finish the current merge, rebase or cherry-pick first")
            }
            val result = CommitRewriter.reword(repo, commitHash, newMessage, JGitCommitSigner.fromConfig(repo))
            CommitRewriter.moveHead(repo, result, "reword: " + newMessage.lineSequence().first())
        }
    }
}
