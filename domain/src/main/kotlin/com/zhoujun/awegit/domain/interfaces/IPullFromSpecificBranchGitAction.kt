package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Branch

interface IPullFromSpecificBranchGitAction {
    suspend operator fun invoke(repositoryPath: String, remoteBranch: Branch, pullWithRebase: Boolean): Either<PullHasConflicts, GitError>
}