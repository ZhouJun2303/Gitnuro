package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.LogStartPoints

interface IGetLogStartPointsGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<LogStartPoints, GitError>
}
