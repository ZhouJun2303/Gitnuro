package com.zhoujun.awegit.data.git.diff

import com.zhoujun.awegit.domain.exceptions.MissingDiffEntryException
import com.zhoujun.awegit.data.git.branches.GetCurrentBranchGitAction
import com.zhoujun.awegit.data.git.repository.GetRepositoryStateGitAction
import com.zhoujun.awegit.domain.errors.okOrNull
import com.zhoujun.awegit.domain.interfaces.IGetDiffEntryFromStatusEntryGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.treewalk.EmptyTreeIterator
import org.eclipse.jgit.treewalk.filter.PathFilter
import javax.inject.Inject

class GetDiffEntryFromStatusEntryGitAction @Inject constructor(
    private val getRepositoryStateGitAction: GetRepositoryStateGitAction,
    private val getCurrentBranchGitAction: GetCurrentBranchGitAction,
) : IGetDiffEntryFromStatusEntryGitAction {
    override suspend operator fun invoke(
        git: Git,
        isCached: Boolean,
        statusEntry: StatusEntry,
    ) = withContext(Dispatchers.IO) {
        val firstDiffEntry = git.diff()
            .setPathFilter(PathFilter.create(statusEntry.filePath))
            .setCached(isCached).apply {
                val repositoryState = getRepositoryStateGitAction(git.repository.directory.absolutePath).okOrNull()!!
                if (
                    getCurrentBranchGitAction(git).okOrNull() == null &&
                    !repositoryState.isMerging &&
                    !repositoryState.isRebasing &&
                    isCached
                ) {
                    setOldTree(EmptyTreeIterator()) // Required if the repository is empty
                }
            }
            .call()
            .firstOrNull()
            ?: throw MissingDiffEntryException("Diff entry not found")

        return@withContext firstDiffEntry
    }
}