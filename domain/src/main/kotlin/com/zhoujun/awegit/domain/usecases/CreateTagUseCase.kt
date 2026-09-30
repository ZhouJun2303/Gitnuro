package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.ICreateTagGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class CreateTagUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val createTagGitAction: ICreateTagGitAction,
) {
    operator fun invoke(
        tag: String,
        revCommit: Commit,
        message: String? = null,
        pushToOrigin: Boolean = false,
    ) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.CreateTag,
            dataToRefresh = arrayOf(DataToRefresh.LOG, DataToRefresh.TAGS, DataToRefresh.REMOTES),
        ) { repositoryPath ->
            createTagGitAction(repositoryPath, tag, revCommit, message, pushToOrigin)
        }
    }
}