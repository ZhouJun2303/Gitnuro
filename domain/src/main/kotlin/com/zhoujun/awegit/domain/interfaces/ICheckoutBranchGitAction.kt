package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Branch

interface ICheckoutBranchGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        branch: Branch,
        localName: String? = null,
        track: Boolean = true,
    ): Either<Unit, GitError>
}