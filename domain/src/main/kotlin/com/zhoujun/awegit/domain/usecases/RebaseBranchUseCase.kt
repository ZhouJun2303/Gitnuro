package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.interfaces.IRebaseBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.TaskType
import com.zhoujun.awegit.domain.models.positiveNotification
import com.zhoujun.awegit.domain.models.warningNotification
import javax.inject.Inject

class RebaseBranchUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val rebaseBranchGitAction: IRebaseBranchGitAction,
) {
    operator fun invoke(branch: Branch) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.RebaseBranch,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            rebaseBranchGitAction(repositoryPath, branch)
        }
    }
}