package com.zhoujun.awegit.data.git.workspace

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IStageEntryGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import com.zhoujun.awegit.domain.models.StatusType
import javax.inject.Inject

class StageEntryGitAction @Inject constructor(
    private val jgit: JGit,
) : IStageEntryGitAction {
    override suspend operator fun invoke(repositoryPath: String, statusEntry: StatusEntry) =
        jgit.provide(repositoryPath) { git ->
            git
                .add()
                .addFilepattern(statusEntry.filePath)
                .setUpdate(statusEntry.statusType == StatusType.REMOVED)
                .call()

            Unit
        }
}