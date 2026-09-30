package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.interfaces.IGetCommitFromRebaseLineGitAction
import com.zhoujun.awegit.domain.models.RebaseLine
import javax.inject.Inject

class GetRebaseLinesFullMessageUseCase @Inject constructor(
    private val getCommitFromRebaseLineGitAction: IGetCommitFromRebaseLineGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend operator fun invoke(lines: List<RebaseLine>): Either<List<RebaseLine>, AppError> {
        return useCaseExecutor.execute { repositoryPath ->
            val result = lines.mapNotNull { line ->
                val commit = getCommitFromRebaseLineGitAction(repositoryPath, line.commit, line.shortMessage).bind() ?: return@mapNotNull null
                val fullMessage = commit.message
                line.copy(commit = commit.hash, fullMessage = fullMessage)
            }

            Either.Ok(result)
        }
    }
}