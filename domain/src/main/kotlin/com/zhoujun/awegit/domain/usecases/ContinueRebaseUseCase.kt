package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.interfaces.IContinueRebaseGitAction
import com.zhoujun.awegit.domain.interfaces.IGetRebaseInteractiveStateGitAction
import com.zhoujun.awegit.domain.interfaces.IGetRepositoryStateGitAction
import com.zhoujun.awegit.domain.models.Identity
import com.zhoujun.awegit.domain.models.RebaseInteractiveState
import com.zhoujun.awegit.domain.models.RepositoryState
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class ContinueRebaseUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val continueRebaseGitAction: IContinueRebaseGitAction,
    private val doCommitUseCase: DoCommitUseCase,
) {
    operator fun invoke(
        message: String,
        isAmendRebaseInteractive: Boolean,
        repositoryState: RepositoryState,
        rebaseInteractiveState: RebaseInteractiveState,
        onIdentityRequest: suspend () -> Identity?,
    ) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.ContinueRebase,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            if (
                repositoryState == RepositoryState.REBASING_INTERACTIVE &&
                rebaseInteractiveState is RebaseInteractiveState.ProcessingCommits &&
                rebaseInteractiveState.isCurrentStepAmenable &&
                isAmendRebaseInteractive
            ) {
                val amendCommitId = rebaseInteractiveState.commitToAmendId

                if (!amendCommitId.isNullOrBlank()) {
                    doCommitUseCase(message, true, onIdentityRequest())
                }
            }

            continueRebaseGitAction(repositoryPath)
        }
    }
}