package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.models.Branch
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.Ref

interface IGetCurrentBranchGitAction {
    suspend operator fun invoke(git: Git): Either<Branch?, AppError>

    suspend operator fun invoke(path: String): Either<Branch?, AppError>
}