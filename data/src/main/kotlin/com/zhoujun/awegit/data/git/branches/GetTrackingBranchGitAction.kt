package com.zhoujun.awegit.data.git.branches

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.BranchesConstants
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.interfaces.IGetTrackingBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.TrackingBranch
import org.eclipse.jgit.lib.Config
import org.eclipse.jgit.lib.Repository
import javax.inject.Inject

class GetTrackingBranchGitAction @Inject constructor(
    private val jgit: JGit,
) : IGetTrackingBranchGitAction {
    override suspend operator fun invoke(repositoryPath: String, branch: Branch): Either<TrackingBranch?, GitError> {
        return this.invoke(repositoryPath, branch.simpleName)
    }

    override suspend operator fun invoke(repositoryPath: String, refName: String) =
        jgit.provide(repositoryPath) { git ->
            val repository: Repository = git.repository

            val config: Config = repository.config
            val remote: String? = config.getString("branch", refName, "remote")
            val branch: String? = config.getString("branch", refName, "merge")

            if (remote != null && branch != null) {
                TrackingBranch(remote, branch.removePrefix(BranchesConstants.UPSTREAM_BRANCH_CONFIG_PREFIX))
            } else {
                null
            }
        }
}

