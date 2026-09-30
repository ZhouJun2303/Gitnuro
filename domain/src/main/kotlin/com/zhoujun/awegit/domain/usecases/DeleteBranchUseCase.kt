package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IDeleteBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class DeleteBranchUseCase @Inject constructor(
    private val deleteBranchGitAction: IDeleteBranchGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(branch: Branch, force: Boolean = true) {
        useCaseExecutor.executeLaunch(
            TaskType.DeleteBranch,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            deleteBranchGitAction(repositoryPath, branch, force)
        }
    }
}