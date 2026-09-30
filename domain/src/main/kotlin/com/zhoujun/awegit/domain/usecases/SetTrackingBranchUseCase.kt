package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IGetTrackingBranchGitAction
import com.zhoujun.awegit.domain.interfaces.ISetTrackingBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class SetTrackingBranchUseCase @Inject constructor(
    private val setTrackingBranchGitAction: ISetTrackingBranchGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(branch: Branch, remoteName: String?, remoteBranch: Branch?) = useCaseExecutor.executeLaunchAsync(
        taskType = TaskType.ChangeBranchUpstream,
        dataToRefresh = arrayOf(DataToRefresh.LOG, DataToRefresh.BRANCHES),
    ) {repositoryPath ->
        setTrackingBranchGitAction(repositoryPath, branch, remoteName, remoteBranch)
    }
}