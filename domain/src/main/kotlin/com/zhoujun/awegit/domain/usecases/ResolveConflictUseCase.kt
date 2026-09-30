package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IResolveConflictGitAction
import javax.inject.Inject

class ResolveConflictUseCase @Inject constructor(
    private val resolveConflictGitAction: IResolveConflictGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend fun read(path: String) = useCaseExecutor.execute { repositoryPath ->
        resolveConflictGitAction.read(repositoryPath, path)
    }

    suspend fun useMine(path: String) = useCaseExecutor.execute(
        dataToRefresh = arrayOf(DataToRefresh.STATUS),
    ) { repositoryPath ->
        resolveConflictGitAction.useStage(repositoryPath, path, 2)
    }

    suspend fun useTheirs(path: String) = useCaseExecutor.execute(
        dataToRefresh = arrayOf(DataToRefresh.STATUS),
    ) { repositoryPath ->
        resolveConflictGitAction.useStage(repositoryPath, path, 3)
    }

    suspend fun markResolved(path: String) = useCaseExecutor.execute(
        dataToRefresh = arrayOf(DataToRefresh.STATUS, DataToRefresh.LOG),
    ) { repositoryPath ->
        resolveConflictGitAction.markResolved(repositoryPath, path)
    }
}
