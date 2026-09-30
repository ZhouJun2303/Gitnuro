package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IResetToCommitGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class ResetBranchUseCase @Inject constructor(
    private val resetToCommitGitAction: IResetToCommitGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(revCommit: Commit, resetType: ResetType) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.ResetToCommit,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            resetToCommitGitAction(repositoryPath, revCommit, resetType = resetType)
        }
    }
}



enum class ResetType {
    SOFT,
    MIXED,
    HARD,
}