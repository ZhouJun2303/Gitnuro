package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IStageEntryGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

private const val TAG = "StatusStageUseCase"

class StatusStageUseCase @Inject constructor(
    private val stageEntryGitAction: IStageEntryGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(statusEntry: StatusEntry) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.StageFile,
            dataToRefresh = arrayOf(DataToRefresh.STATUS),
        ) { repositoryPath ->
            stageEntryGitAction(repositoryPath, statusEntry)
        }
    }
}
