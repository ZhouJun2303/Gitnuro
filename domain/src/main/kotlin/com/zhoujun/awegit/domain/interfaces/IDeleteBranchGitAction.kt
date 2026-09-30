package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Branch

interface IDeleteBranchGitAction {
    suspend operator fun invoke(repositoryPath: String, branch: Branch, force: Boolean = true): Either<Unit, GitError>
}