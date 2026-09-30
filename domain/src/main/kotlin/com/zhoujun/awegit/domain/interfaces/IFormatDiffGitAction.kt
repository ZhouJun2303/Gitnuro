package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.DiffResult
import com.zhoujun.awegit.domain.models.DiffType
import org.eclipse.jgit.api.Git

interface IFormatDiffGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        diffType: DiffType,
        isDisplayFullFile: Boolean,
    ): Either<DiffResult, GitError>
}