package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IDeleteSubmoduleGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class DeleteSubmoduleUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val deleteSubmoduleGitAction: IDeleteSubmoduleGitAction,
) {
    operator fun invoke(path: String) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.DeleteSubmodule,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            deleteSubmoduleGitAction(repositoryPath, path)
        }
    }
}