package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.interfaces.IWorktreeGitAction
import com.zhoujun.awegit.domain.models.WorktreeListResult
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import javax.inject.Inject

class ListWorktreesUseCase @Inject constructor(
    private val worktreeGitAction: IWorktreeGitAction,
    private val repositoryDataRepository: RepositoryDataRepository,
) {
    suspend operator fun invoke(): WorktreeListResult {
        val repositoryPath = repositoryDataRepository.repositoryPath
            ?: return WorktreeListResult(gitAvailable = true, worktrees = emptyList())
        return when (val result = worktreeGitAction.list(repositoryPath)) {
            is Either.Ok -> result.value
            is Either.Err -> WorktreeListResult(gitAvailable = false, worktrees = emptyList())
        }
    }
}

class RemoveWorktreeUseCase @Inject constructor(
    private val worktreeGitAction: IWorktreeGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend operator fun invoke(path: String) = useCaseExecutor.execute(
        dataToRefresh = arrayOf(DataToRefresh.ALL),
    ) { repositoryPath ->
        worktreeGitAction.remove(repositoryPath, path)
    }
}
