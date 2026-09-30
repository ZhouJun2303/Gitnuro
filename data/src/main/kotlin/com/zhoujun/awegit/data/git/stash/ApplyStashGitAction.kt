package com.zhoujun.awegit.data.git.stash

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IApplyStashGitAction
import com.zhoujun.awegit.domain.models.Commit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.Git
import javax.inject.Inject

class ApplyStashGitAction @Inject constructor(
    private val jgit: JGit,
) : IApplyStashGitAction {
    override suspend operator fun invoke(repositoryPath: String, stashInfo: Commit) = jgit.provide(repositoryPath) { git ->
        invoke(git, stashInfo)
    }

    operator fun invoke(git: Git, stashInfo: Commit) {
        git.stashApply()
            .setStashRef(stashInfo.hash)
            .call()
    }
}