package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IDeleteSubmoduleGitAction
import com.zhoujun.awegit.domain.interfaces.IUpdateSubmoduleGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class UpdateSubmoduleUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val updateSubmoduleGitAction: IUpdateSubmoduleGitAction,
) {
    operator fun invoke(path: String) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.UpdateSubmodule,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            updateSubmoduleGitAction(repositoryPath, path)
        }
    }
}