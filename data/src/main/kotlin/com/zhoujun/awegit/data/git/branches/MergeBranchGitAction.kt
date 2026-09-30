package com.zhoujun.awegit.data.git.branches

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.exceptions.UncommittedChangesDetectedException
import com.zhoujun.awegit.domain.interfaces.IMergeBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import org.eclipse.jgit.api.MergeCommand
import org.eclipse.jgit.api.MergeResult
import org.eclipse.jgit.lib.ObjectId
import javax.inject.Inject


class MergeBranchGitAction @Inject constructor(
    private val jgit: JGit,
) : IMergeBranchGitAction {
    /**
     * @return true if success has conflicts, false if success without conflicts
     */
    override suspend operator fun invoke(
        repositoryPath: String,
        branch: Branch,
        fastForward: Boolean,
        squash: Boolean = false,
        fastForwardOnly: Boolean = false,
    ) = jgit.provide(repositoryPath) { git ->

        val fastForwardMode = when {
            fastForwardOnly -> MergeCommand.FastForwardMode.FF_ONLY
            fastForward -> MergeCommand.FastForwardMode.FF
            else -> MergeCommand.FastForwardMode.NO_FF
        }

        val mergeBase: ObjectId = git.repository.resolve(branch.name) ?: throw Exception("Branch ${branch.name} not found")
        val currentBranch = git.repository.branch.orEmpty()

        val mergeResult = git
            .merge()
            .include(mergeBase)
            .setFastForward(fastForwardMode)
            .setSquash(squash)
            .setMessage("Merge branch '${branch.simpleNameWithRemote}' into $currentBranch")
            .call()

        if (mergeResult.mergeStatus == MergeResult.MergeStatus.FAILED) {
            throw UncommittedChangesDetectedException("Merge failed, makes sure you repository doesn't contain uncommitted changes.")
        }

        val hasConflicts = mergeResult.mergeStatus == MergeResult.MergeStatus.CONFLICTING

        hasConflicts
    }
}