package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GenericError
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.errors.either
import com.zhoujun.awegit.domain.errors.raiseError
import com.zhoujun.awegit.domain.interfaces.ICheckHasUncommittedChangesGitAction
import com.zhoujun.awegit.domain.interfaces.IGetStashListGitAction
import com.zhoujun.awegit.domain.interfaces.IPopStashGitAction
import com.zhoujun.awegit.domain.interfaces.IPullBranchGitAction
import com.zhoujun.awegit.domain.interfaces.IStageUntrackedFileGitAction
import com.zhoujun.awegit.domain.interfaces.IStashChangesGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.PullOptions
import com.zhoujun.awegit.domain.models.PullType
import com.zhoujun.awegit.domain.models.TaskType
import kotlinx.coroutines.Job
import javax.inject.Inject

private const val AUTO_STASH_MESSAGE = "AweGit: auto stash before pull"

class PullWithOptionsUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val checkHasUncommittedChangesGitAction: ICheckHasUncommittedChangesGitAction,
    private val stageUntrackedFileGitAction: IStageUntrackedFileGitAction,
    private val stashChangesGitAction: IStashChangesGitAction,
    private val getStashListGitAction: IGetStashListGitAction,
    private val pullBranchGitAction: IPullBranchGitAction,
    private val popStashGitAction: IPopStashGitAction,
) {
    operator fun invoke(options: PullOptions): Job = useCaseExecutor.executeLaunch(
        taskType = TaskType.Pull,
        dataToRefresh = arrayOf(DataToRefresh.ALL),
        refreshEvenIfFailed = true,
    ) { repositoryPath -> pull(repositoryPath, options) }

    /** Exposed so unit tests can call the pull flow without starting a task. */
    suspend fun pull(repositoryPath: String, options: PullOptions): Either<Unit, AppError> = either {
        val hasChanges = checkHasUncommittedChangesGitAction(repositoryPath).bind()
        val stash: Commit? = if (options.stashAndReapply && hasChanges) {
            stageUntrackedFileGitAction(repositoryPath).bind()
            stashChangesGitAction(repositoryPath, AUTO_STASH_MESSAGE).bind()
            getStashListGitAction(repositoryPath).bind().firstOrNull()
        } else {
            null
        }

        val result = pullBranchGitAction(
            repositoryPath = repositoryPath,
            pullType = if (options.rebase) PullType.REBASE else PullType.MERGE,
            mergeAutoStash = false,
            remoteBranch = options.remoteBranch,
            automaticStashDescription = AUTO_STASH_MESSAGE,
        )

        when (result) {
            is Either.Err -> {
                if (stash != null) popStashGitAction(repositoryPath, stash)
                Either.Err(result.error)
            }

            is Either.Ok -> when {
                result.value && stash != null -> raiseError(
                    GenericError("Pull produced conflicts. Your local changes were saved as stash \"$AUTO_STASH_MESSAGE\" and were not reapplied. Resolve the conflicts first, then apply that stash.")
                )

                stash != null -> when (popStashGitAction(repositoryPath, stash)) {
                    is Either.Ok -> Either.Ok(Unit)
                    is Either.Err -> raiseError(
                        GenericError("Pull completed, but reapplying your local changes failed. They are kept as stash \"$AUTO_STASH_MESSAGE\".")
                    )
                }

                else -> Either.Ok(Unit)
            }
        }
    }
}
