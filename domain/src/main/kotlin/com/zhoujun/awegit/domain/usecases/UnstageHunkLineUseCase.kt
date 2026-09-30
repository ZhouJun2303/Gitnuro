package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IUnstageHunkLineGitAction
import com.zhoujun.awegit.domain.models.Hunk
import com.zhoujun.awegit.domain.models.Line
import com.zhoujun.awegit.domain.models.TaskType
import org.eclipse.jgit.diff.DiffEntry
import javax.inject.Inject

class UnstageHunkLineUseCase @Inject constructor(
    private val unstageHunkLineGitAction: IUnstageHunkLineGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(diffEntry: DiffEntry, hunk: Hunk, line: Line) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.UnstageLine,
            dataToRefresh = arrayOf(DataToRefresh.STATUS),
        ) { repositoryPath ->
            unstageHunkLineGitAction(repositoryPath, diffEntry, hunk, line)
        }
    }
}
