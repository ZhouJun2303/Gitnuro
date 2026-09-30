package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.GraphLogGenerator
import com.zhoujun.awegit.domain.LogGenerationSupersededException
import com.zhoujun.awegit.domain.Pagination
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.LogGenerationSupersededError
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.errors.either
import com.zhoujun.awegit.domain.errors.raiseError
import com.zhoujun.awegit.domain.interfaces.IGetLogStartPointsGitAction
import com.zhoujun.awegit.domain.interfaces.IGetStatusGitAction
import com.zhoujun.awegit.domain.models.GraphCommits
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import javax.inject.Inject
import kotlin.math.max

private const val INITIAL_COMMITS_LOAD = 2000

class GetLogUseCase @Inject constructor(
    private val getLogStartPointsGitAction: IGetLogStartPointsGitAction,
    private val getStatusGitAction: IGetStatusGitAction,
    private val graphLogGenerator: GraphLogGenerator,
    private val repositoryDataRepository: RepositoryDataRepository,
) {
    suspend operator fun invoke(repository: String, pagination: Pagination) = either<GraphCommits, AppError> {
        val startPoints = getLogStartPointsGitAction(repository).bind()
        val status = repositoryDataRepository.latestStatus ?: getStatusGitAction(repository).bind()
        val hasUncommittedChanges = status.staged.isNotEmpty() || status.unstaged.isNotEmpty()
        try {
            Either.Ok(
                graphLogGenerator.generate(
                    repository,
                    maxCommits = max(repositoryDataRepository.maxCommitsToLoadLimit, INITIAL_COMMITS_LOAD),
                    hashes = startPoints.hashes,
                    stashes = HashSet(startPoints.stashes),
                    forcedFirstLaneBranchHash = if (hasUncommittedChanges) startPoints.headHash else null,
                    pagination = pagination,
                )
            )
        } catch (e: LogGenerationSupersededException) {
            raiseError(LogGenerationSupersededError)
        }
    }
}
