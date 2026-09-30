package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IDiscardUnstagedHunkLineGitAction
import com.zhoujun.awegit.domain.models.Hunk
import com.zhoujun.awegit.domain.models.Line
import com.zhoujun.awegit.domain.models.TaskType
import org.eclipse.jgit.diff.DiffEntry
import javax.inject.Inject

class DiscardHunkLineUseCase @Inject constructor(
    private val discardUnstagedHunkLineGitAction: IDiscardUnstagedHunkLineGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(diffEntry: DiffEntry, hunk: Hunk, line: Line) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.Unspecified,
            dataToRefresh = arrayOf(DataToRefresh.STATUS),
        ) { repositoryPath ->
            discardUnstagedHunkLineGitAction(repositoryPath, diffEntry, hunk, line)
        }
    }
}
