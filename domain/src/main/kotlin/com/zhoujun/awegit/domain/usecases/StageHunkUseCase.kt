package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IStageHunkGitAction
import com.zhoujun.awegit.domain.models.Hunk
import com.zhoujun.awegit.domain.models.TaskType
import org.eclipse.jgit.diff.DiffEntry
import javax.inject.Inject

class StageHunkUseCase @Inject constructor(
    private val stageHunkGitAction: IStageHunkGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(diffEntry: DiffEntry, hunk: Hunk) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.StageHunk,
            dataToRefresh = arrayOf(DataToRefresh.STATUS),
        ) { repositoryPath ->
            stageHunkGitAction(repositoryPath, diffEntry, hunk)
        }
    }
}
