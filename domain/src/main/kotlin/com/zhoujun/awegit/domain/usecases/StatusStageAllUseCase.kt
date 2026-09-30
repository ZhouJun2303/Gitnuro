package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IStageAllGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

private const val TAG = "StatusStageAllUseCase"

class StatusStageAllUseCase @Inject constructor(
    private val stageAllGitAction: IStageAllGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(entries: List<StatusEntry>?) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.StageAllFiles,
            dataToRefresh = arrayOf(DataToRefresh.STATUS),
        ) { repositoryPath ->
            stageAllGitAction(repositoryPath, entries)
        }
    }
}
