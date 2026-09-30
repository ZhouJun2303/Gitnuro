package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.models.StatusEntry

interface IStageEntryGitAction {
    suspend operator fun invoke(repositoryPath: String, statusEntry: StatusEntry): Either<Unit, AppError>
}