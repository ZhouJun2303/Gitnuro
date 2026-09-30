package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.models.Status

interface IGetStatusGitAction {
    suspend operator fun invoke(repository: String, paths: List<String> = emptyList()): Either<Status, AppError>
}