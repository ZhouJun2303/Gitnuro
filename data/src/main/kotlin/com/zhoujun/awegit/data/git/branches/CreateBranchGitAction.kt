package com.zhoujun.awegit.data.git.branches

import com.zhoujun.awegit.common.extensions.runIfNotNull
import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.errors.CreateBranchError
import com.zhoujun.awegit.domain.errors.GenericError
import com.zhoujun.awegit.domain.errors.mapErr
import com.zhoujun.awegit.domain.interfaces.ICreateBranchGitAction
import com.zhoujun.awegit.domain.models.Commit
import javax.inject.Inject

class CreateBranchGitAction @Inject constructor(
    private val jgit: JGit,
) : ICreateBranchGitAction {
    override suspend operator fun invoke(
        repositoryPath: String,
        branchName: String,
        targetCommit: Commit?,
        checkout: Boolean,
    ) =
        jgit.provide(repositoryPath) { git ->
            if (checkout) {
                git
                    .checkout()
                    .setCreateBranch(true)
                    .setName(branchName)
                    .runIfNotNull(targetCommit) { commit ->
                        setStartPoint(commit.hash)
                    }
                    .call()
            } else {
                git
                    .branchCreate()
                    .setName(branchName)
                    .runIfNotNull(targetCommit) { commit ->
                        setStartPoint(commit.hash)
                    }
                    .call()
            }

            Unit
        }.mapErr {
            if (it is GenericError) {
                if (it.message == "Ref $branchName already exists") {
                    CreateBranchError.BranchAlreadyExists(branchName)
                } else if (it.message == "Branch name $branchName is not allowed") {
                    CreateBranchError.NameNotAllowed(branchName)
                } else {
                    it
                }
            } else {
                it
            }
        }
}