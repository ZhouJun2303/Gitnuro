package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IFetchAllRemotesGitAction
import com.zhoujun.awegit.domain.models.Remote
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class FetchAllBranchUseCase @Inject constructor(
    private val fetchAllGitAction: IFetchAllRemotesGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(specificRemote: Remote? = null) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.Fetch,
            refreshEvenIfFailed = true,
            dataToRefresh = arrayOf(DataToRefresh.LOG),
        ) { repositoryPath ->
            fetchAllGitAction(repositoryPath, specificRemote)
        }
    }
}