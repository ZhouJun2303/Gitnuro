package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.LfsError

interface IProvideLfsCredentialsGitAction {
    suspend operator fun <T> invoke(
        url: String,
        callback: suspend (username: String?, password: String?) -> Either<T, LfsError>,
    ): Either<T, LfsError>
}