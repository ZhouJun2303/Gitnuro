package com.zhoujun.awegit.data.git.submodules

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IInitializeSubmoduleGitAction
import javax.inject.Inject

class InitializeSubmoduleGitAction @Inject constructor(
    private val jgit: JGit,
) : IInitializeSubmoduleGitAction {
    override suspend operator fun invoke(repositoryPath: String, path: String) = jgit.provide(repositoryPath) { git ->
        git.submoduleInit()
            .addPath(path)
            .call()

        Unit
    }
}