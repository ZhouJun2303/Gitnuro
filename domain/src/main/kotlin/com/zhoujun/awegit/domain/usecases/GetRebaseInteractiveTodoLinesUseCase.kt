package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.interfaces.IGetRebaseInteractiveTodoLinesGitAction
import com.zhoujun.awegit.domain.models.RebaseLine
import javax.inject.Inject

class GetRebaseInteractiveTodoLinesUseCase @Inject constructor(
    private val getRebaseInteractiveTodoLinesGitAction: IGetRebaseInteractiveTodoLinesGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend operator fun invoke(): Either<List<RebaseLine>, AppError> {
        return useCaseExecutor.execute(
        ) { repositoryPath ->
            getRebaseInteractiveTodoLinesGitAction(repositoryPath)
        }
    }
}