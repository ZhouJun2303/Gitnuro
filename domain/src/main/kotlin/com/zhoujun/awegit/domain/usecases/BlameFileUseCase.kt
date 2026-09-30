package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.errors.RepositoryPathNotSetError
import com.zhoujun.awegit.domain.interfaces.IBlameFileGitAction
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import org.eclipse.jgit.blame.BlameResult
import javax.inject.Inject

class BlameFileUseCase @Inject constructor(
    private val repositoryDataRepository: RepositoryDataRepository,
    private val blameFileGitAction: IBlameFileGitAction,
) {
    suspend operator fun invoke(filePath: String): Either<BlameResult, GitError> {
        val repositoryPath = repositoryDataRepository.repositoryPath ?: return Either.Err(RepositoryPathNotSetError)
        return blameFileGitAction(repositoryPath, filePath)
    }
}