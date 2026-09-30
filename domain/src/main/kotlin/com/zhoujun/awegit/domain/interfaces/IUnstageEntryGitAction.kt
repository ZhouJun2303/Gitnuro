package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.models.StatusEntry
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.Ref

interface IUnstageEntryGitAction {
    suspend operator fun invoke(repositoryPath: String, statusEntry: StatusEntry): Either<Unit, AppError>
}