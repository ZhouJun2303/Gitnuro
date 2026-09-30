package com.zhoujun.awegit.data.git.log

import org.eclipse.jgit.lib.Constants
import org.eclipse.jgit.lib.Repository
import org.eclipse.jgit.revwalk.RevCommit
import org.eclipse.jgit.revwalk.RevWalk

object UnpushedCommitsCalculator {
    const val MAX_TRACKED = 5_000

    fun compute(repository: Repository, limit: Int = MAX_TRACKED): Set<String> {
        val head = repository.resolve(Constants.HEAD) ?: return emptySet()
        RevWalk(repository).use { walk ->
            walk.markStart(walk.parseCommit(head))
            for (ref in repository.refDatabase.getRefsByPrefix(Constants.R_REMOTES)) {
                val id = ref.peeledObjectId ?: ref.objectId ?: continue
                val obj = runCatching { walk.parseAny(id) }.getOrNull()
                if (obj is RevCommit) walk.markUninteresting(obj)
            }
            val result = LinkedHashSet<String>()
            for (commit in walk) {
                result.add(commit.name)
                if (result.size >= limit) break
            }
            return result
        }
    }
}
