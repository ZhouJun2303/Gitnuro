package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.Pagination
import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.LogGenerationSupersededError
import com.zhoujun.awegit.domain.repositories.DataState
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import javax.inject.Inject

class IncreaseLogCountUseCase @Inject constructor(
    private val repositoryDataRepository: RepositoryDataRepository,
    private val useCaseExecutor: UseCaseExecutor,
    private val getLogUseCase: GetLogUseCase,
) {
    suspend operator fun invoke(newLimit: Int): Either<Unit, AppError> {
        return useCaseExecutor.execute { repositoryPath ->
            // If the data is being loaded or failed, do not try to load more items
            val log = (repositoryDataRepository.log.value as? DataState.Loaded)?.data ?: return@execute Either.Ok(Unit)

            if (newLimit > repositoryDataRepository.maxCommitsToLoadLimit) {
                repositoryDataRepository.maxCommitsToLoadLimit = newLimit
                val result = getLogUseCase(repositoryPath, pagination = Pagination.Paginated(log))
                if (result is Either.Err && result.error is LogGenerationSupersededError) return@execute Either.Ok(Unit)
                repositoryDataRepository.updateLog { result }
            }

            Either.Ok(Unit)
        }
    }
}