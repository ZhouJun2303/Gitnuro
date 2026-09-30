package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.extensions.nullIfEmpty
import com.zhoujun.awegit.domain.interfaces.IStageUntrackedFileGitAction
import com.zhoujun.awegit.domain.interfaces.IStashChangesGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class StashChangesUseCase @Inject constructor(
    private val stageUntrackedFileGitAction: IStageUntrackedFileGitAction,
    private val stashChangesGitAction: IStashChangesGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(message: String?) = useCaseExecutor.executeLaunch(
        taskType = TaskType.Stash,
        dataToRefresh = arrayOf(DataToRefresh.STATUS, DataToRefresh.STASHES, DataToRefresh.LOG),
    ) { repositoryPath ->
        stageUntrackedFileGitAction(repositoryPath).bind()

        stashChangesGitAction(repositoryPath, message?.nullIfEmpty)
    }
}