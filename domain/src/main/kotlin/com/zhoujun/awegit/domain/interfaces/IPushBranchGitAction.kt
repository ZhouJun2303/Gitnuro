package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Branch

interface IPushBranchGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        force: Boolean,
        pushTags: Boolean,
        pushWithLease: Boolean,
        specificBranch: Branch? = null,
        sourceBranch: Branch? = null,
        setUpstream: Boolean = false,
    ): Either<Unit, GitError>
}