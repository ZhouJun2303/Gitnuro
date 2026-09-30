package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IAbortRebaseGitAction
import com.zhoujun.awegit.domain.interfaces.IResetRepositoryStateGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class ResetRepositoryStateUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val resetRepositoryStateGitAction: IResetRepositoryStateGitAction,
) {
    operator fun invoke() {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.ResetRepoState,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            resetRepositoryStateGitAction(repositoryPath)
        }
    }
}
