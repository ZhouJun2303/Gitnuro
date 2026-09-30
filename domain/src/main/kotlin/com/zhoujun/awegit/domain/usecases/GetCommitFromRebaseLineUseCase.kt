package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.interfaces.IGetCommitFromRebaseLineGitAction
import com.zhoujun.awegit.domain.models.Commit
import javax.inject.Inject

class GetCommitFromRebaseLineUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val getCommitFromRebaseGitAction: IGetCommitFromRebaseLineGitAction,
) {
    suspend operator fun invoke(commitShortHash: String, shortMessage: String): Either<Commit?, AppError> {
        return useCaseExecutor.execute { repositoryPath ->
            getCommitFromRebaseGitAction(repositoryPath, commitShortHash, shortMessage)
        }
    }
}