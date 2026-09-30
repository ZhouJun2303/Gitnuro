package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IUnstageAllGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class StatusUnstageAllUseCase @Inject constructor(
    private val unstageAllGitAction: IUnstageAllGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(statusEntries: List<StatusEntry>?) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.UnstageAllFiles,
            dataToRefresh = arrayOf(DataToRefresh.STATUS),
        ) { repositoryPath ->
            unstageAllGitAction(repositoryPath, statusEntries)
        }
    }
}
