package com.zhoujun.awegit.data.git.branches

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IDeleteBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import javax.inject.Inject

class DeleteBranchGitAction @Inject constructor(private val jgit: JGit) : IDeleteBranchGitAction {
    override suspend operator fun invoke(repositoryPath: String, branch: Branch, force: Boolean) = jgit.provide(repositoryPath) { git ->
        git
            .branchDelete()
            .setBranchNames(branch.name)
            .setForce(force)
            .call()

        Unit
    }
}