package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.interfaces.IDeleteFileGitAction
import com.zhoujun.awegit.domain.interfaces.IDiscardEntriesGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import com.zhoujun.awegit.domain.models.TaskType
import java.io.File
import javax.inject.Inject

class DeleteFileUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val deleteFileGitAction: IDeleteFileGitAction,
) {
    operator fun invoke(filePath: String) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.DiscardFile,
            dataToRefresh = arrayOf(DataToRefresh.STATUS, DataToRefresh.LOG),
        ) { repositoryPath ->
            deleteFileGitAction(repositoryPath, filePath)
        }
    }
}
