package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IPushBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.TaskType
import com.zhoujun.awegit.domain.services.AppSettingsService
import kotlinx.coroutines.flow.first
import javax.inject.Inject

class PushBranchUseCase @Inject constructor(
    private val pushBranchGitAction: IPushBranchGitAction,
    private val appSettingsService: AppSettingsService,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(
        force: Boolean,
        pushTags: Boolean,
        targetRemoteBranch: Branch? = null,
        sourceBranch: Branch? = null,
        setUpstream: Boolean = false,
    ) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.Push,
            dataToRefresh = arrayOf(DataToRefresh.LOG, DataToRefresh.REMOTES),
        ) { repositoryPath ->
            val pushWithLease = appSettingsService.pushWithLease.first()

            pushBranchGitAction(
                repositoryPath,
                force,
                pushTags,
                pushWithLease,
                targetRemoteBranch,
                sourceBranch,
                setUpstream,
            )
        }
    }
}