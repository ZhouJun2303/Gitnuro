package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.ICherryPickCommitGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class CherryPickCommitUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val cherryPickGitAction: ICherryPickCommitGitAction,
) {
    operator fun invoke(commit: Commit) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.CherryPickCommit,
            dataToRefresh = arrayOf(DataToRefresh.STATUS, DataToRefresh.LOG, DataToRefresh.BRANCHES, DataToRefresh.REPO_STATE),
        ) { repositoryPath ->
            cherryPickGitAction(repositoryPath, commit)
        }
    }
}