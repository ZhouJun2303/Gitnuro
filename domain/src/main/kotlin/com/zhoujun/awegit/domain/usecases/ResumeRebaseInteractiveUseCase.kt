package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IResumeRebaseInteractiveGitAction
import com.zhoujun.awegit.domain.models.TaskType
import org.eclipse.jgit.api.RebaseCommand
import javax.inject.Inject

class ResumeRebaseInteractiveUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val resumeRebaseInteractiveGitAction: IResumeRebaseInteractiveGitAction,
) {
    operator fun invoke(interactiveHandler: RebaseCommand.InteractiveHandler) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.RebaseInteractive,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            resumeRebaseInteractiveGitAction(repositoryPath, interactiveHandler)
        }
    }
}