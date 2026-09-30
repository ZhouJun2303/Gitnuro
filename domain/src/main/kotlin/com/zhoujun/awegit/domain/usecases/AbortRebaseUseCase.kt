package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IAbortRebaseGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class AbortRebaseUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val abortRebaseGitAction: IAbortRebaseGitAction,
) {
    operator fun invoke() {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.AbortRebase,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            abortRebaseGitAction(repositoryPath)
        }
    }
}