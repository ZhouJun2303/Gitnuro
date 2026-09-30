package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.usecases.ResetType
import org.eclipse.jgit.api.Git

interface IResetToCommitGitAction {
    suspend operator fun invoke(repositoryPath: String, commit: Commit, resetType: ResetType): Either<Unit, GitError>
}