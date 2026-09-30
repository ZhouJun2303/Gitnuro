package com.zhoujun.awegit.data.git.branches

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.JGitBranchMapper
import com.zhoujun.awegit.domain.interfaces.IRenameBranchGitAction
import javax.inject.Inject

class RenameBranchGitAction @Inject constructor(
    private val jGitBranchMapper: JGitBranchMapper,
    private val jgit: JGit,
) : IRenameBranchGitAction {
    override suspend operator fun invoke(repositoryPath: String, oldName: String, newName: String) =
        jgit.provide(repositoryPath) { git ->
            val ref = git.branchRename()
                .setOldName(oldName)
                .setNewName(newName)
                .call()

            checkNotNull(jGitBranchMapper.toDomain(ref)) { "Failed to map $ref to domain branch" }
        }
}