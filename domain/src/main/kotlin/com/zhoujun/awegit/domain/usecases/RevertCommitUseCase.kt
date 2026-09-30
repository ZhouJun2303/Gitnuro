package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IRevertCommitGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class RevertCommitUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val revertCommitGitAction: IRevertCommitGitAction,
) {
    operator fun invoke(commit: Commit) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.RevertCommit,
            refreshEvenIfFailed = true,
            dataToRefresh = arrayOf(DataToRefresh.STATUS, DataToRefresh.LOG),
        ) { repositoryPath ->
            revertCommitGitAction(repositoryPath, commit)
        }
    }
}