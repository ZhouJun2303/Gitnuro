package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.models.DiffContent
import org.eclipse.jgit.diff.DiffEntry
import org.eclipse.jgit.lib.Repository
import org.eclipse.jgit.treewalk.AbstractTreeIterator

interface IGetDiffContentGitAction {
    operator fun invoke(
        repository: Repository,
        diffEntry: DiffEntry,
        oldTreeIterator: AbstractTreeIterator?,
        newTreeIterator: AbstractTreeIterator?,
    ): DiffContent
}