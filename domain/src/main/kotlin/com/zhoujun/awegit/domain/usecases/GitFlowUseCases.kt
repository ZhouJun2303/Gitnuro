package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IGitFlowGitAction
import com.zhoujun.awegit.domain.models.GitFlowBranchType
import com.zhoujun.awegit.domain.models.GitFlowConfig
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class LoadGitFlowConfigUseCase @Inject constructor(
    private val gitFlowGitAction: IGitFlowGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend operator fun invoke() = useCaseExecutor.execute { repositoryPath ->
        gitFlowGitAction.load(repositoryPath)
    }
}

class SaveGitFlowConfigUseCase @Inject constructor(
    private val gitFlowGitAction: IGitFlowGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(config: GitFlowConfig) = useCaseExecutor.executeLaunch(
        taskType = TaskType.GitFlowInit,
        dataToRefresh = arrayOf(DataToRefresh.GIT_CONFIG),
    ) { repositoryPath ->
        gitFlowGitAction.save(repositoryPath, config)
    }
}

class GitFlowInitUseCase @Inject constructor(
    private val gitFlowGitAction: IGitFlowGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(config: GitFlowConfig) = useCaseExecutor.executeLaunch(
        taskType = TaskType.GitFlowInit,
        dataToRefresh = arrayOf(DataToRefresh.BRANCHES, DataToRefresh.GIT_CONFIG),
    ) { repositoryPath ->
        gitFlowGitAction.init(repositoryPath, config)
    }
}

class GitFlowStartUseCase @Inject constructor(
    private val gitFlowGitAction: IGitFlowGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(type: GitFlowBranchType, name: String) = useCaseExecutor.executeLaunch(
        taskType = TaskType.GitFlowStart,
        dataToRefresh = arrayOf(DataToRefresh.BRANCHES, DataToRefresh.LOG),
    ) { repositoryPath ->
        gitFlowGitAction.start(repositoryPath, type, name)
    }
}

class GitFlowFinishUseCase @Inject constructor(
    private val gitFlowGitAction: IGitFlowGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(type: GitFlowBranchType, name: String, deleteBranch: Boolean, tagMessage: String) =
        useCaseExecutor.executeLaunch(
            taskType = TaskType.GitFlowFinish,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            gitFlowGitAction.finish(repositoryPath, type, name, deleteBranch, tagMessage)
        }
}
