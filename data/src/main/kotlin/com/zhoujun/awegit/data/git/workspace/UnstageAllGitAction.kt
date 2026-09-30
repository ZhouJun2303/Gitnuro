package com.zhoujun.awegit.data.git.workspace

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IUnstageAllGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import javax.inject.Inject

class UnstageAllGitAction @Inject constructor(
    private val jgit: JGit,
) : IUnstageAllGitAction {
    override suspend operator fun invoke(repositoryPath: String, entries: List<StatusEntry>?) =
        jgit.provide(repositoryPath) { git ->
            git
                .reset()
                .apply {
                    entries?.forEach { entry ->
                        addPath(entry.filePath)
                    }
                }
                .call()

            Unit
        }
}