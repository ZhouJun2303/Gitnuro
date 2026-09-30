package com.zhoujun.awegit.data.git.branches

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.ICheckoutBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import org.eclipse.jgit.api.CreateBranchCommand
import javax.inject.Inject

class CheckoutBranchGitAction @Inject constructor(
    private val jgit: JGit,
) : ICheckoutBranchGitAction {
    override suspend operator fun invoke(
        repositoryPath: String,
        branch: Branch,
        localName: String?,
        track: Boolean,
    ) = jgit.provide(repositoryPath) { git ->
        git.checkout().apply {
            if (branch.name.startsWith("refs/remotes/")) {
                val checkoutName = localName?.takeIf { it.isNotBlank() } ?: branch.simpleName
                setCreateBranch(true)
                setName(checkoutName)
                setStartPoint(branch.name)
                setUpstreamMode(
                    if (track) {
                        CreateBranchCommand.SetupUpstreamMode.SET_UPSTREAM
                    } else {
                        CreateBranchCommand.SetupUpstreamMode.NOTRACK
                    },
                )
            } else {
                setName(branch.name)
            }
            call()
        }

        Unit
    }
}