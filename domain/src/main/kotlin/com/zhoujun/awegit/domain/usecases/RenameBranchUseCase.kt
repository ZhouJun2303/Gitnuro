package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.interfaces.IRenameBranchGitAction
import com.zhoujun.awegit.domain.interfaces.ISetTrackingBranchGitAction
import javax.inject.Inject

class RenameBranchUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val renameBranchGitAction: IRenameBranchGitAction,
    private val setTrackingBranchGitAction: ISetTrackingBranchGitAction,
) {
    suspend operator fun invoke(oldName: String, newName: String): Either<Unit, AppError> {
        return useCaseExecutor.execute(
            dataToRefresh = arrayOf(DataToRefresh.BRANCHES, DataToRefresh.LOG),
        ) { repositoryPath ->
            val branch = renameBranchGitAction(repositoryPath, oldName, newName).bind()

            setTrackingBranchGitAction(repositoryPath, branch, null, null)
        }
    }
}