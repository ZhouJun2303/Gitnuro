package com.zhoujun.awegit.data.git.stash

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.errors.StashChangesError
import com.zhoujun.awegit.domain.errors.raiseError
import com.zhoujun.awegit.domain.interfaces.IStashChangesGitAction
import javax.inject.Inject

class StashChangesGitAction @Inject constructor(
    private val jgit: JGit,
) : IStashChangesGitAction {
    override suspend operator fun invoke(repositoryPath: String, message: String?) = jgit.provide(repositoryPath) { git ->
        val commit = git
            .stashCreate()
            .setIncludeUntracked(true)
            .apply {
                if (message != null)
                    setWorkingDirectoryMessage(message)
            }
            .call()


        if (commit == null) {
            raiseError(StashChangesError.NoDataToStash)
        }
    }
}