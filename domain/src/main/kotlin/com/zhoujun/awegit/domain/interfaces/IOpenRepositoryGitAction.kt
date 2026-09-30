package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import org.eclipse.jgit.lib.Repository
import java.io.File

interface IOpenRepositoryGitAction {
    suspend operator fun invoke(directory: String): Either<String, AppError>
}