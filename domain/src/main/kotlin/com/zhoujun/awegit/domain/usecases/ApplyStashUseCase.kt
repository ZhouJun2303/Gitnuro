package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IApplyStashGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class ApplyStashUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val applyStashGitAction: IApplyStashGitAction,
) {
    operator fun invoke(stashCommit: Commit) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.ApplyStash,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
            refreshEvenIfFailed = true,
        ) { repositoryPath ->
            applyStashGitAction(repositoryPath, stashCommit)
        }
    }
}