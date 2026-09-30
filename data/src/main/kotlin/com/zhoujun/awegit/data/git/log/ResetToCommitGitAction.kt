package com.zhoujun.awegit.data.git.log

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IResetToCommitGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.usecases.ResetType
import org.eclipse.jgit.api.ResetCommand
import javax.inject.Inject

class ResetToCommitGitAction @Inject constructor(
    private val jgit: JGit,
) : IResetToCommitGitAction {
    override suspend operator fun invoke(repositoryPath: String, commit: Commit, resetType: ResetType) =
        jgit.provide(repositoryPath) { git ->
            val reset = when (resetType) {
                ResetType.SOFT -> ResetCommand.ResetType.SOFT
                ResetType.MIXED -> ResetCommand.ResetType.MIXED
                ResetType.HARD -> ResetCommand.ResetType.HARD
            }
            git
                .reset()
                .setMode(reset)
                .setRef(commit.hash)
                .call()

            Unit
        }
}
