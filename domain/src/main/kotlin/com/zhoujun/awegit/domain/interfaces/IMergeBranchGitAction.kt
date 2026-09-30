package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Branch

interface IMergeBranchGitAction {
    /**
     * @return true if success has conflicts, false if success without conflicts
     */
    suspend operator fun invoke(
        repositoryPath: String,
        branch: Branch,
        fastForward: Boolean,
        squash: Boolean = false,
        fastForwardOnly: Boolean = false,
    ): Either<Boolean, GitError>
}