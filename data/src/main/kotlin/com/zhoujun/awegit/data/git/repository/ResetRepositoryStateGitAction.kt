package com.zhoujun.awegit.data.git.repository

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IResetRepositoryStateGitAction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.api.ResetCommand
import javax.inject.Inject

class ResetRepositoryStateGitAction @Inject constructor(
    private val jgit: JGit,
) : IResetRepositoryStateGitAction {
    override suspend operator fun invoke(repositoryPath: String) = jgit.provide(repositoryPath) { git ->
        git.repository.apply {
            writeMergeCommitMsg(null)
            writeMergeHeads(null)
        }

        git.reset()
            .setMode(ResetCommand.ResetType.HARD)
            .call()

        Unit
    }
}