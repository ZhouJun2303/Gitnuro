package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.models.StatusEntry
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.diff.DiffEntry

interface IGetDiffEntryFromStatusEntryGitAction {
    suspend operator fun invoke(
        git: Git,
        isCached: Boolean,
        statusEntry: StatusEntry,
    ): DiffEntry
}