package com.zhoujun.awegit.data.git.workspace

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IUnstageEntryGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import javax.inject.Inject

class UnstageEntryGitAction @Inject constructor(
    private val jgit: JGit,
) : IUnstageEntryGitAction {
    override suspend operator fun invoke(repositoryPath: String, statusEntry: StatusEntry) =
        jgit.provide(repositoryPath) { git ->
            git
                .reset()
                .addPath(statusEntry.filePath)
                .call()

            Unit
        }
}