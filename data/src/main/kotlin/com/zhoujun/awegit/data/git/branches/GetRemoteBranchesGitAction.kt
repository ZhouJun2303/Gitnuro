package com.zhoujun.awegit.data.git.branches

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.JGitBranchMapper
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.interfaces.IGetRemoteBranchesGitAction
import com.zhoujun.awegit.domain.models.Branch
import org.eclipse.jgit.api.ListBranchCommand
import javax.inject.Inject

class GetRemoteBranchesGitAction @Inject constructor(
    private val jGitBranchMapper: JGitBranchMapper,
    private val jgit: JGit,
) : IGetRemoteBranchesGitAction {
    override suspend operator fun invoke(repositoryPath: String): Either<List<Branch>, GitError> {
        return jgit.provide(repositoryPath) { git ->
            git
                .branchList()
                .setListMode(ListBranchCommand.ListMode.REMOTE)
                .call()
                .mapNotNull { jGitBranchMapper.toDomain(it) }
        }
    }
}