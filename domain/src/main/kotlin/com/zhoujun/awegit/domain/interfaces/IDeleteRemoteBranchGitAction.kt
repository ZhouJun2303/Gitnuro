package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Branch

interface IDeleteRemoteBranchGitAction {
    suspend operator fun invoke(repositoryPath: String, ref: Branch): Either<Unit, GitError>
}