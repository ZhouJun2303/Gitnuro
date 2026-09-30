package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IStageHunkLineGitAction
import com.zhoujun.awegit.domain.models.Hunk
import com.zhoujun.awegit.domain.models.Line
import com.zhoujun.awegit.domain.models.TaskType
import org.eclipse.jgit.diff.DiffEntry
import javax.inject.Inject

class StageHunkLineUseCase @Inject constructor(
    private val stageHunkLineGitAction: IStageHunkLineGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(diffEntry: DiffEntry, hunk: Hunk, line: Line) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.StageLine,
            dataToRefresh = arrayOf(DataToRefresh.STATUS),
        ) { repositoryPath ->
            stageHunkLineGitAction(repositoryPath, diffEntry, hunk, line)
        }
    }
}
