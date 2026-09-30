package com.zhoujun.awegit.data.git.rebase

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.RebaseConstants
import com.zhoujun.awegit.domain.interfaces.IGetRebaseInteractiveStateGitAction
import com.zhoujun.awegit.domain.models.RebaseInteractiveState
import java.io.File
import javax.inject.Inject

class GetRebaseInteractiveStateGitAction @Inject constructor(
    private val getRebaseAmendCommitIdGitAction: GetRebaseAmendCommitIdGitAction,
    private val jgit: JGit,
) : IGetRebaseInteractiveStateGitAction {
    override suspend operator fun invoke(repositoryPath: String) = jgit.provide(repositoryPath) { git ->
        // TODO Delete this action
        TODO()
    }
}