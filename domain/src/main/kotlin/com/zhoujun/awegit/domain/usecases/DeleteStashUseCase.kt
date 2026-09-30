package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IDeleteStashGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class DeleteStashUseCase @Inject constructor(
    private val deleteStashGitAction: IDeleteStashGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(stash: Commit) = useCaseExecutor.executeLaunch(
        taskType = TaskType.Stash,
        refreshEvenIfFailed = true,
        dataToRefresh = arrayOf(DataToRefresh.STASHES, DataToRefresh.LOG),
    ) { repositoryPath ->
        deleteStashGitAction(repositoryPath, stash)
    }
}