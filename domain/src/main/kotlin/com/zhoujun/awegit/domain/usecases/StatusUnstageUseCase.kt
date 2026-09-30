package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IUnstageEntryGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

private const val TAG = "StatusUnstageUseCase"


class StatusUnstageUseCase @Inject constructor(
    private val unstageEntryGitAction: IUnstageEntryGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(statusEntry: StatusEntry) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.UnstageFile,
            dataToRefresh = arrayOf(DataToRefresh.STATUS),
        ) { repositoryPath ->
            unstageEntryGitAction(repositoryPath, statusEntry)
        }
    }
}
