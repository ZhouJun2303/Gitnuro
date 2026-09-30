package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.GenericError
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.errors.raiseError
import com.zhoujun.awegit.domain.interfaces.IGetUnpushedCommitsGitAction
import com.zhoujun.awegit.domain.interfaces.IRewordCommitGitAction
import com.zhoujun.awegit.domain.models.TaskType
import kotlinx.coroutines.Job
import javax.inject.Inject

class RewordCommitUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val rewordCommitGitAction: IRewordCommitGitAction,
    private val getUnpushedCommitsGitAction: IGetUnpushedCommitsGitAction,
) {
    operator fun invoke(commitHash: String, newMessage: String): Job = useCaseExecutor.executeLaunch(
        taskType = TaskType.RewordCommit,
        dataToRefresh = arrayOf(DataToRefresh.BRANCHES, DataToRefresh.LOG),
    ) { repositoryPath ->
        val message = newMessage.trimEnd()
        if (message.isBlank()) raiseError(GenericError("Commit message can't be empty"))
        if (commitHash !in getUnpushedCommitsGitAction(repositoryPath).bind()) {
            raiseError(GenericError("Only commits that haven't been pushed can be edited"))
        }
        rewordCommitGitAction(repositoryPath, commitHash, message)
    }
}
