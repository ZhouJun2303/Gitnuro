package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IFetchAllRemotesGitAction
import com.zhoujun.awegit.domain.models.Remote
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class FetchWithOptionsUseCase @Inject constructor(
    private val fetchAllRemotesGitAction: IFetchAllRemotesGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(remote: Remote?, fetchAllTags: Boolean, prune: Boolean) =
        useCaseExecutor.executeLaunch(
            taskType = TaskType.Fetch,
            dataToRefresh = arrayOf(DataToRefresh.REMOTES, DataToRefresh.BRANCHES, DataToRefresh.TAGS, DataToRefresh.LOG),
        ) { repositoryPath ->
            fetchAllRemotesGitAction(repositoryPath, remote, fetchAllTags, prune)
        }
}
