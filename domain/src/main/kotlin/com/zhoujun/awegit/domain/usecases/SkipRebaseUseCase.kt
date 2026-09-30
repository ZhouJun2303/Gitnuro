package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IAbortRebaseGitAction
import com.zhoujun.awegit.domain.interfaces.ISkipRebaseGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class SkipRebaseUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val skipRebaseGitAction: ISkipRebaseGitAction,
) {
    operator fun invoke() {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.SkipRebase,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            skipRebaseGitAction(repositoryPath)
        }
    }
}