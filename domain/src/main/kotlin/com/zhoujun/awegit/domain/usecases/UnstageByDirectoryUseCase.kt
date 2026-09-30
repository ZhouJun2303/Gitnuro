package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IUnstageByDirectoryGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class UnstageByDirectoryUseCase @Inject constructor(
    private val unstageByDirectoryGitAction: IUnstageByDirectoryGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(dir: String) = useCaseExecutor.executeLaunch(
        taskType = TaskType.StageDir,
        dataToRefresh = arrayOf(DataToRefresh.STATUS),
    ) { repositoryPath ->
        unstageByDirectoryGitAction(repositoryPath, dir)
    }
}