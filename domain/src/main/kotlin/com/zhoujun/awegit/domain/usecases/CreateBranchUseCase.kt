package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.ICreateBranchGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class CreateBranchUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    val createBranchGitAction: ICreateBranchGitAction,
) {
    operator fun invoke(branchName: String, target: Commit?, checkout: Boolean = true) {
        // TODO Should be "execute" and handle the result in the UI
        useCaseExecutor.executeLaunch(
            taskType = TaskType.CreateBranch,
            refreshEvenIfFailed = true, // TODO Previously this was a conditional lambda: refreshEvenIfCrashesInteractive = { it is CheckoutConflictException },
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            createBranchGitAction(repositoryPath, branchName, target, checkout)
        }
    }
}