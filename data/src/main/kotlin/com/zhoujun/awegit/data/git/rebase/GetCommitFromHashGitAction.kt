package com.zhoujun.awegit.data.git.rebase

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.JGitCommitMapper
import com.zhoujun.awegit.domain.interfaces.IGetCommitFromHashGitAction
import org.eclipse.jgit.lib.ObjectId
import javax.inject.Inject

class GetCommitFromHashGitAction @Inject constructor(
    private val commitMapper: JGitCommitMapper,
    private val jgit: JGit,
) : IGetCommitFromHashGitAction {
    override suspend operator fun invoke(repositoryPath: String, commitHash: String) =
        jgit.provide(repositoryPath) { git ->
            git
                .repository
                .parseCommit(ObjectId.fromString(commitHash))
                ?.let { commit -> commitMapper.toDomain(commit) }
        }
}