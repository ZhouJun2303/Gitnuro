package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IStartRebaseInteractiveGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class StartRebaseInteractiveUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val startRebaseInteractiveGitAction: IStartRebaseInteractiveGitAction,
) {
    operator fun invoke(commit: Commit) =  useCaseExecutor.executeLaunch(
        taskType = TaskType.RebaseInteractive,
        dataToRefresh = arrayOf(DataToRefresh.REPO_STATE),
    ) { repositoryPath ->
        startRebaseInteractiveGitAction(repositoryPath, commit)
    }
}