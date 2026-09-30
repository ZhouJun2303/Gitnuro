package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.errors.RepositoryPathNotSetError
import com.zhoujun.awegit.domain.interfaces.ILoadSignOffConfigGitAction
import com.zhoujun.awegit.domain.interfaces.ISaveLocalRepositoryConfigGitAction
import com.zhoujun.awegit.domain.models.SignOffConfig
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import javax.inject.Inject

class SaveSignOffConfigUseCase @Inject constructor(
    private val repositoryDataRepository: RepositoryDataRepository,
    private val saveLocalRepositoryConfigGitAction: ISaveLocalRepositoryConfigGitAction,
) {
    suspend operator fun invoke(signOffConfig: SignOffConfig): Either<Unit, GitError> {
        val repositoryPath = repositoryDataRepository.repositoryPath ?: return Either.Err(RepositoryPathNotSetError)
        return saveLocalRepositoryConfigGitAction(repositoryPath, signOffConfig)
    }
}