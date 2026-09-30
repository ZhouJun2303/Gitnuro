package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IDiscardEntriesGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class DiscardEntriesUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val discardEntriesGitAction: IDiscardEntriesGitAction,
) {
    operator fun invoke(statusEntries: List<StatusEntry>, isStaged: Boolean) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.DiscardFile,
            dataToRefresh = arrayOf(DataToRefresh.STATUS, DataToRefresh.LOG),
        ) { repositoryPath ->
            discardEntriesGitAction(repositoryPath, statusEntries, isStaged)
        }
    }
}
