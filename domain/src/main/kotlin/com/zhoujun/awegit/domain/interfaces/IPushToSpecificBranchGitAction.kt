package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Branch

interface IPushToSpecificBranchGitAction {
    suspend operator fun invoke(repositoryPath: String, force: Boolean, pushTags: Boolean, remoteBranch: Branch): Either<Unit, GitError>
}