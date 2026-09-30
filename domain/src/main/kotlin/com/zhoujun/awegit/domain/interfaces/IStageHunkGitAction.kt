package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Hunk
import org.eclipse.jgit.diff.DiffEntry

interface IStageHunkGitAction {
    suspend operator fun invoke(repositoryPath: String, diffEntry: DiffEntry, hunk: Hunk): Either<Unit, GitError>
}