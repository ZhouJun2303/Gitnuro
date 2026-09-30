package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IStageByDirectoryGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class StageByDirectoryUseCase @Inject constructor(
    private val stageByDirectoryGitAction: IStageByDirectoryGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(dir: String) = useCaseExecutor.executeLaunch(
        taskType = TaskType.StageDir,
        dataToRefresh = arrayOf(DataToRefresh.STATUS),
    ) { repositoryPath ->
        stageByDirectoryGitAction(repositoryPath, dir)
    }
}