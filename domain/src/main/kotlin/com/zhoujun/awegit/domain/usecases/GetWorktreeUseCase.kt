package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.interfaces.IGetWorktreePathGitAction
import javax.inject.Inject

class GetWorktreeUseCase @Inject constructor(
    private val getWorktreePathGitAction: IGetWorktreePathGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend operator fun invoke(): Either<String, AppError> {
        return useCaseExecutor.execute(
        ) { repositoryPath ->
            getWorktreePathGitAction(repositoryPath)
        }
    }
}