package com.zhoujun.awegit.domain.interfaces

import org.eclipse.jgit.api.PullResult

typealias PullHasConflicts = Boolean

interface IHasPullResultConflictsGitAction {
    operator fun invoke(isRebase: Boolean, pullResult: PullResult): PullHasConflicts
}