package com.zhoujun.awegit.data.git.submodules

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.ISyncSubmoduleGitAction
import javax.inject.Inject

class SyncSubmoduleGitAction @Inject constructor(
    private val jgit: JGit,
) : ISyncSubmoduleGitAction {
    override suspend operator fun invoke(
        repositoryPath: String,
        path: String,
    ) = jgit.provide(repositoryPath) { git ->
        git.submoduleSync()
            .addPath(path)
            .call()

        Unit
    }
}