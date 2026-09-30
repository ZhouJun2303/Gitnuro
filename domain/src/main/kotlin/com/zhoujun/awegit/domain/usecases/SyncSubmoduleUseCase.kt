package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.ISyncSubmoduleGitAction
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class SyncSubmoduleUseCase @Inject constructor(
    private val syncSubmoduleGitAction: ISyncSubmoduleGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(submodulePath: String) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.SyncSubmodule,
            dataToRefresh = arrayOf(DataToRefresh.SUBMODULES),
        ) { repository ->
            syncSubmoduleGitAction(repository, submodulePath)
        }
    }
}