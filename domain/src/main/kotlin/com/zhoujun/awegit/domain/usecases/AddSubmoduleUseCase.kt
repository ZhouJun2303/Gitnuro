package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IAddSubmoduleGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class AddSubmoduleUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val addSubmoduleGitAction: IAddSubmoduleGitAction,
) {
    operator fun invoke(name: String, path: String, uri: String) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.AddSubmodule,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            addSubmoduleGitAction(repositoryPath, name, path, uri)
        }
    }
}