package com.zhoujun.awegit.data.git.stash

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.JGitCommitMapper
import com.zhoujun.awegit.domain.interfaces.IGetStashListGitAction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.revwalk.RevCommit
import javax.inject.Inject

class GetStashListGitAction @Inject constructor(
    private val commitMapper: JGitCommitMapper,
    private val jgit: JGit,
) : IGetStashListGitAction {
    override suspend operator fun invoke(repositoryPath: String) = jgit.provide(repositoryPath) { git ->
        invoke(git)
            .map { commitMapper.toDomain(it) }
    }

    suspend operator fun invoke(git: Git): List<RevCommit> = withContext(Dispatchers.IO) {
        return@withContext git
            .stashList()
            .call()
            .toList()
    }
}
