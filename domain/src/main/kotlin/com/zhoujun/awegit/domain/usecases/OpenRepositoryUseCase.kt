package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.common.measureAndLog
import com.zhoujun.awegit.domain.AppStateManager
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.okOrNull
import com.zhoujun.awegit.domain.interfaces.IOpenRepositoryGitAction
import com.zhoujun.awegit.domain.models.RepositorySelectionState
import com.zhoujun.awegit.domain.models.TaskType
import com.zhoujun.awegit.domain.repositories.FailureSeverity
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.RepositoryStateRepository
import javax.inject.Inject

class OpenRepositoryUseCase @Inject constructor(
    private val repositoryDataRepository: RepositoryDataRepository,
    private val repositoryStateRepository: RepositoryStateRepository,
    private val openRepositoryGitAction: IOpenRepositoryGitAction,
    private val refreshDataUseCase: RefreshDataUseCase,
    private val observeRepositoryToRefreshUseCase: ObserveRepositoryToRefreshUseCase,
    private val getWorktreeUseCase: GetWorktreeUseCase,
    private val appStateManager: AppStateManager,
) {
    suspend operator fun invoke(directory: String) {
        val repositoryPathResult = openRepositoryGitAction(directory)

        when (repositoryPathResult) {
            is Either.Err ->  {
                repositoryDataRepository.setRepositorySelectionState(RepositorySelectionState.None)
                repositoryStateRepository.addCompletedTaskFailed(
                    TaskType.RepositoryOpen,
                    repositoryPathResult.error,
                    FailureSeverity.HIGH,
                )
            }
            is Either.Ok -> {
                measureAndLog("OpenRepositoryUseCase", "open until first log") {
                    repositoryDataRepository.setRepositorySelectionState(RepositorySelectionState.Open(repositoryPathResult.value))

                    val worktree = getWorktreeUseCase().okOrNull()
                    if (worktree != null) {
                        appStateManager.repositoryTabChanged(worktree)
                    }

                    refreshDataUseCase(DataToRefresh.ALL).join()
                }
                observeRepositoryToRefreshUseCase()
            }
        }
    }
}