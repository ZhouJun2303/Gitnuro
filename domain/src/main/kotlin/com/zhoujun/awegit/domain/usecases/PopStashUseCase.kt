package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.GenericError
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.errors.raiseError
import com.zhoujun.awegit.domain.interfaces.IGetStashListGitAction
import com.zhoujun.awegit.domain.interfaces.IPopStashGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class PopStashUseCase @Inject constructor(
    private val popStashGitAction: IPopStashGitAction,
    private val getStashListGitAction: IGetStashListGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(commit: Commit?) = useCaseExecutor.executeLaunch(
        taskType = TaskType.PopStash,
        refreshEvenIfFailed = true,
        dataToRefresh = arrayOf(DataToRefresh.STATUS, DataToRefresh.LOG, DataToRefresh.STASHES),
    ) { repositoryPath ->
        val stashCommit = commit ?: getStashListGitAction(repositoryPath).bind().firstOrNull()

        if (stashCommit == null) {
            raiseError(GenericError("No stashes found")) // TODO Refactor this to a proper type
        }

        popStashGitAction(repositoryPath, stashCommit)
    }
}