package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.ICheckoutBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class CheckoutBranchUseCase @Inject constructor(
    private val checkoutBranchGitAction: ICheckoutBranchGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(branch: Branch, localName: String? = null, track: Boolean = true) {
        useCaseExecutor.executeLaunch(
            taskType = if (branch.isRemote) TaskType.CheckoutRemoteBranch else TaskType.CheckoutBranch,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            checkoutBranchGitAction(repositoryPath, branch, localName, track)
        }
    }
}