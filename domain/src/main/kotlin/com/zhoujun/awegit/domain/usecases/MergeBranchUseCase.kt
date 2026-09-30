package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.interfaces.ICheckHasUncommittedChangesGitAction
import com.zhoujun.awegit.domain.interfaces.ICreateSnapshotStashGitAction
import com.zhoujun.awegit.domain.interfaces.IDeleteStashGitAction
import com.zhoujun.awegit.domain.interfaces.IMergeBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.TaskType
import com.zhoujun.awegit.domain.services.AppSettingsService
import kotlinx.coroutines.flow.first
import javax.inject.Inject

enum class MergeMode { Default, NoFastForward, FastForwardOnly, Squash }

class MergeBranchUseCase @Inject constructor(
    private val mergeBranchGitAction: IMergeBranchGitAction,
    private val appSettingsService: AppSettingsService,
    private val checkHasUncommittedChangesGitAction: ICheckHasUncommittedChangesGitAction,
    private val useCaseExecutor: UseCaseExecutor,
    private val deleteStashGitAction: IDeleteStashGitAction,
    private val createSnapshotStashGitAction: ICreateSnapshotStashGitAction,
) {

    operator fun invoke(branch: Branch, automaticStashDescription: String, mode: MergeMode = MergeMode.Default) {
        useCaseExecutor.executeLaunch(
            TaskType.MergeBranch,
            refreshEvenIfFailed = true,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            val mergeAutoStash = appSettingsService.autoStashOnMerge.first()
            val fastForwardMerge = when (mode) {
                MergeMode.Default -> appSettingsService.fastForwardMerge.first()
                MergeMode.NoFastForward, MergeMode.Squash -> false
                MergeMode.FastForwardOnly -> true
            }
            var backupStash: Commit? = null

            if (mergeAutoStash) {
                val hasUncommitedChanges = checkHasUncommittedChangesGitAction(repositoryPath).bind()
                if (hasUncommitedChanges) {
                    backupStash = createSnapshotStashGitAction(
                        repositoryPath,
                        message = automaticStashDescription,
                        includeUntracked = true
                    ).bind()
                }
            }

            val result = mergeBranchGitAction(
                repositoryPath,
                branch,
                fastForwardMerge,
                squash = mode == MergeMode.Squash,
                fastForwardOnly = mode == MergeMode.FastForwardOnly,
            )

            if (result is Either.Ok) {
                val hasConflicts = result.value

                if (!hasConflicts && backupStash != null) {
                    deleteStashGitAction(repositoryPath, backupStash)
                }
            }

            result
        }
    }
}