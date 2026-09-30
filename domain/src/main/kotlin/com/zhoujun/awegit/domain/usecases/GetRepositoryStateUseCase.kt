package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.errors.RepositoryPathNotSetError
import com.zhoujun.awegit.domain.interfaces.IGetRepositoryStateGitAction
import com.zhoujun.awegit.domain.models.RepositoryState
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import javax.inject.Inject

class GetRepositoryStateUseCase @Inject constructor(
    private val gitRepositoryStateGitAction: IGetRepositoryStateGitAction,
    private val repositoryDataRepository: RepositoryDataRepository,
) {
    suspend operator fun invoke(): Either<RepositoryState, GitError> {
        val repositoryPath = repositoryDataRepository.repositoryPath ?: return Either.Err(RepositoryPathNotSetError)
        return gitRepositoryStateGitAction(repositoryPath)
    }
}