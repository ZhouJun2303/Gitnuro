package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IUnstageHunkGitAction
import com.zhoujun.awegit.domain.models.Hunk
import com.zhoujun.awegit.domain.models.TaskType
import org.eclipse.jgit.diff.DiffEntry
import javax.inject.Inject

class UnstageHunkUseCase @Inject constructor(
    private val unstageHunkGitAction: IUnstageHunkGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(diffEntry: DiffEntry, hunk: Hunk) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.UnstageHunk,
            dataToRefresh = arrayOf(DataToRefresh.STATUS),
        ) { repositoryPath ->
            unstageHunkGitAction(repositoryPath, diffEntry, hunk)
        }
    }
}
