package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.models.StatusEntry
import org.eclipse.jgit.api.Git

interface IUnstageAllGitAction {
    suspend operator fun invoke(repositoryPath: String, entries: List<StatusEntry>?): Either<Unit, AppError>
}