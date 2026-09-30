package com.zhoujun.awegit.domain

import com.zhoujun.awegit.domain.errors.*
import com.zhoujun.awegit.domain.extensions.runOperationInTabScope
import com.zhoujun.awegit.domain.extensions.runOperationInTabScopeAsync
import com.zhoujun.awegit.domain.models.TaskType
import com.zhoujun.awegit.domain.repositories.FailureSeverity
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.RepositoryStateRepository
import com.zhoujun.awegit.domain.usecases.DataToRefresh
import com.zhoujun.awegit.domain.usecases.RefreshDataUseCase
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Provider

class UseCaseExecutor @Inject constructor(
    private val repositoryDataRepository: RepositoryDataRepository,
    private val repositoryStateRepository: RepositoryStateRepository,
    private val refreshDataUseCase: Provider<RefreshDataUseCase>,
    private val scope: TabCoroutineScope,
) {
    suspend fun <T> execute(
        dataToRefresh: Array<DataToRefresh> = emptyArray(),
        refreshEvenIfFailed: Boolean = false,
        block: suspend EitherContext<AppError>.(String) -> Either<T, AppError>,
    ): Either<T, AppError> {
        return executeTask(dataToRefresh = dataToRefresh, refreshEvenIfFailed, block)
    }
    suspend fun executeWithoutResult(
        dataToRefresh: Array<DataToRefresh> = emptyArray(),
        refreshEvenIfFailed: Boolean = false,
        block: suspend EitherContext<AppError>.(String) -> Unit,
    ) {
        executeTask(dataToRefresh = dataToRefresh, refreshEvenIfFailed) {
            this@executeTask.block(it)
            Either.Ok(Unit)
        }
    }

    fun executeOnTabScope(
        block: suspend EitherContext<AppError>.(String) -> Unit,
    ) {
        scope.launch {
            executeTask(
                refreshEvenIfFailed = false,
                dataToRefresh = emptyArray(),
                block = {
                    block(it)
                    Either.Ok(Unit)
                }
            )
        }
    }

    fun <T> executeLaunch(
        taskType: TaskType,
        dataToRefresh: Array<DataToRefresh>,
        refreshEvenIfFailed: Boolean = false,
        isForegroundTask: Boolean = true,
        block: suspend EitherContext<AppError>.(String) -> Either<T, AppError>,
    ): Job {
        return repositoryStateRepository.runOperationInTabScope(taskType, scope, isForegroundTask) {
            executeTaskWithErrorHandling(
                taskType,
                dataToRefresh,
                refreshEvenIfFailed,
                block,
            )
        }
    }

    fun <T> executeLaunchAsync(
        taskType: TaskType,
        dataToRefresh: Array<DataToRefresh>,
        refreshEvenIfFailed: Boolean = false,
        isForegroundTask: Boolean = true,
        block: suspend EitherContext<AppError>.(String) -> Either<T, AppError>,
    ): Deferred<Either<T, AppError>> {
        return repositoryStateRepository.runOperationInTabScopeAsync(taskType, scope, isForegroundTask) {
            executeTaskWithErrorHandling(
                taskType,
                dataToRefresh,
                refreshEvenIfFailed,
                block,
            )
        }
    }

    private suspend fun <T> executeTaskWithErrorHandling(
        taskType: TaskType,
        dataToRefresh: Array<DataToRefresh>,
        refreshEvenIfFailed: Boolean = false,
        block: suspend EitherContext<AppError>.(String) -> Either<T, AppError>,
    ): Either<T, AppError> {
        return executeTask(
            dataToRefresh = dataToRefresh,
            refreshEvenIfFailed,
            block
        ).apply {
            when (this) {
                is Either.Err -> repositoryStateRepository.addCompletedTaskFailed(
                    taskType,
                    this.error,
                    FailureSeverity.HIGH,
                )

                is Either.Ok -> repositoryStateRepository.addCompletedTaskSuccessfully(taskType)
            }
        }
    }

    private suspend fun <T> executeTask(
        dataToRefresh: Array<DataToRefresh>,
        refreshEvenIfFailed: Boolean,
        block: suspend EitherContext<AppError>.(String) -> Either<T, AppError>
    ): Either<T, AppError> {
        try {
            val repositoryPath = repositoryDataRepository.repositoryPath ?: return Either.Err(RepositoryPathNotSetError)
            return either { block(repositoryPath) }.apply {
                if (this is Either.Ok || refreshEvenIfFailed) {
                    if (dataToRefresh.isNotEmpty()) {
                        refreshDataUseCase.get()(*dataToRefresh)
                    }
                }
            }
        } catch (e: Exception) {
            return Either.Err(GenericError(e.message.orEmpty(), e))
        }
    }
}