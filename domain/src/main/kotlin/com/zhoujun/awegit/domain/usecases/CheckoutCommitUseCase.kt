package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.ICheckoutCommitGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class CheckoutCommitUseCase @Inject constructor(
    private val checkoutCommitGitAction: ICheckoutCommitGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(commit: Commit) {
        invoke(commit.hash)
    }

    operator fun invoke(hash: String) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.CheckoutCommit,
            dataToRefresh = arrayOf(DataToRefresh.STATUS, DataToRefresh.LOG, DataToRefresh.BRANCHES),
        ) { repositoryPath ->
            checkoutCommitGitAction(repositoryPath, hash)
        }
    }
}