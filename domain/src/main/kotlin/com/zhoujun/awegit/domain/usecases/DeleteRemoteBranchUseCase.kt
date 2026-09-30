package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IDeleteBranchGitAction
import com.zhoujun.awegit.domain.interfaces.IDeleteRemoteBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class DeleteRemoteBranchUseCase @Inject constructor(
    private val deleteRemoteBranchGitAction: IDeleteRemoteBranchGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(branch: Branch) {
        useCaseExecutor.executeLaunch(
            TaskType.DeleteBranch,
            dataToRefresh = arrayOf(DataToRefresh.ALL), // TODO Refresh only log?
        ) { repositoryPath ->
            deleteRemoteBranchGitAction(repositoryPath, branch)
        }
    }
}