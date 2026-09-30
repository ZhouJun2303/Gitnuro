package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.interfaces.IGetCommitFromHashGitAction
import com.zhoujun.awegit.domain.repositories.DataState
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import javax.inject.Inject

class GetCommitFromHashUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val getCommitFromHashGitAction: IGetCommitFromHashGitAction,
    private val repositoryDataRepository: RepositoryDataRepository,
) {
    suspend operator fun invoke(commitHash: String) = useCaseExecutor.execute { repositoryPath ->
        val logDataState = repositoryDataRepository.log.value
        val commit = (logDataState as? DataState.Loaded)?.data[commitHash]?.commit

        if (commit != null) {
            return@execute Either.Ok(commit)
        }

        getCommitFromHashGitAction(repositoryPath, commitHash)
    }
}