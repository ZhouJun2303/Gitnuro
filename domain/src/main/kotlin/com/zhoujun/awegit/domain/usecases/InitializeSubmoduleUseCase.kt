package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.interfaces.IInitializeSubmoduleGitAction
import com.zhoujun.awegit.domain.interfaces.IUpdateSubmoduleGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class InitializeSubmoduleUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val initializeSubmoduleGitAction: IInitializeSubmoduleGitAction,
    private val updateSubmoduleGitAction: IUpdateSubmoduleGitAction,
) {
    operator fun invoke(path: String) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.InitSubmodule,
            dataToRefresh = arrayOf(DataToRefresh.SUBMODULES),
        ) { repositoryPath ->
            initializeSubmoduleGitAction(repositoryPath, path).bind()
            updateSubmoduleGitAction(repositoryPath, path)
        }
    }
}