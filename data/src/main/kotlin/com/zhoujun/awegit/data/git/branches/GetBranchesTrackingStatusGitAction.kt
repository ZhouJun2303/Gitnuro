package com.zhoujun.awegit.data.git.branches

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IGetBranchesTrackingStatusGitAction
import com.zhoujun.awegit.domain.models.TrackingCounts
import javax.inject.Inject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.lib.BranchTrackingStatus
import org.eclipse.jgit.lib.Repository

class GetBranchesTrackingStatusGitAction @Inject constructor(
    private val jgit: JGit,
) : IGetBranchesTrackingStatusGitAction {
    override suspend fun invoke(repositoryPath: String) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            val map = mutableMapOf<String, TrackingCounts>()
            for (ref in git.branchList().call()) {
                val name = Repository.shortenRefName(ref.name)
                val status = BranchTrackingStatus.of(git.repository, name) ?: continue
                map[name] = TrackingCounts(status.aheadCount, status.behindCount)
            }
            map
        }
    }
}
