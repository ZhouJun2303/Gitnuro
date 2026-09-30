package com.zhoujun.awegit.data.git.branches

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.JGitBranchMapper
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.interfaces.IGetBranchesGitAction
import com.zhoujun.awegit.domain.models.Branch
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.Git
import javax.inject.Inject

class GetBranchesGitAction @Inject constructor(
    private val jGitBranchMapper: JGitBranchMapper,
    private val jgit: JGit,
) : IGetBranchesGitAction {
    // TODO after refactor remove this overload
    override suspend operator fun invoke(git: Git): Either<List<Branch>, AppError> {
        return invoke(git.repository.directory.absolutePath)
    }

    override suspend operator fun invoke(repository: String): Either<List<Branch>, AppError> {
        return jgit.provide(repository) { git ->
            git
                .branchList()
                .call()
                .mapNotNull { jGitBranchMapper.toDomain(it) }
        }
    }
}