package com.zhoujun.awegit.data.git.log

import org.eclipse.jgit.lib.CommitBuilder
import org.eclipse.jgit.lib.Constants
import org.eclipse.jgit.lib.ObjectId
import org.eclipse.jgit.lib.PersonIdent
import org.eclipse.jgit.lib.RefUpdate
import org.eclipse.jgit.lib.Repository
import org.eclipse.jgit.revwalk.RevSort
import org.eclipse.jgit.revwalk.RevWalk

object CommitRewriter {
    data class Result(val oldHead: ObjectId, val newHead: ObjectId)

    fun reword(repository: Repository, targetHash: String, newMessage: String, signer: CommitSigner? = null): Result {
        val headId = repository.resolve(Constants.HEAD) ?: throw IllegalStateException("Repository has no HEAD")
        val targetId = ObjectId.fromString(targetHash)

        RevWalk(repository).use { check ->
            if (!check.isMergedInto(check.parseCommit(targetId), check.parseCommit(headId))) {
                throw IllegalArgumentException("Commit $targetHash is not an ancestor of HEAD")
            }
        }

        RevWalk(repository).use { walk ->
            val head = walk.parseCommit(headId)
            val target = walk.parseCommit(targetId)
            walk.sort(RevSort.TOPO, true)
            walk.sort(RevSort.REVERSE, true)
            walk.markStart(head)
            for (parent in target.parents) walk.markUninteresting(walk.parseCommit(parent))
            val range = walk.toList()

            val newCommitter = PersonIdent(repository)
            val mapping = HashMap<ObjectId, ObjectId>()
            repository.newObjectInserter().use { inserter ->
                for (commit in range) {
                    val isTarget = commit.id == target.id
                    val oldParents = commit.parents.map { it.toObjectId() }
                    val newParents = oldParents.map { mapping[it] ?: it }
                    if (!isTarget && newParents == oldParents) continue

                    val builder = CommitBuilder().apply {
                        setTreeId(commit.tree)
                        setParentIds(newParents)
                        author = commit.authorIdent
                        committer = newCommitter
                        encoding = runCatching { commit.encoding }.getOrDefault(Charsets.UTF_8)
                        message = if (isTarget) newMessage else commit.fullMessage
                    }
                    signer?.sign(builder, newCommitter)
                    mapping[commit.toObjectId()] = inserter.insert(builder)
                }
                inserter.flush()
            }
            return Result(headId, mapping[headId] ?: headId)
        }
    }

    /** Atomically moves the current branch (or HEAD when detached) and writes reflog plus ORIG_HEAD. */
    fun moveHead(repository: Repository, result: Result, reflogMessage: String) {
        val fullBranch = repository.fullBranch
        val detached = !fullBranch.startsWith(Constants.R_HEADS)
        val update = repository.updateRef(if (detached) Constants.HEAD else fullBranch, detached)
        update.setExpectedOldObjectId(result.oldHead)
        update.setNewObjectId(result.newHead)
        update.setForceUpdate(true)
        update.setRefLogMessage(reflogMessage, false)
        when (val r = update.update()) {
            RefUpdate.Result.FORCED, RefUpdate.Result.FAST_FORWARD, RefUpdate.Result.NEW, RefUpdate.Result.NO_CHANGE -> Unit
            else -> throw IllegalStateException("Could not update ${update.name}: $r")
        }
        repository.writeOrigHead(result.oldHead)
    }

    /**
     * Squashes [hashes] into one commit whose tree is the newest commit and whose parent is the
     * oldest commit's parent. Later commits on the first-parent chain are rewritten to hang off it.
     */
    fun squash(repository: Repository, hashes: List<String>, message: String, signer: CommitSigner? = null): Result {
        if (hashes.isEmpty()) throw IllegalArgumentException("No commits to squash")
        val headId = repository.resolve(Constants.HEAD) ?: throw IllegalStateException("Repository has no HEAD")
        val ids = hashes.map { ObjectId.fromString(it) }

        RevWalk(repository).use { walk ->
            val head = walk.parseCommit(headId)
            val commits = ids.map { walk.parseCommit(it) }
            val oldest = commits.minBy { commit ->
                var depth = 0
                var cursor = commit
                while (cursor.parentCount > 0) {
                    cursor = walk.parseCommit(cursor.getParent(0))
                    depth++
                }
                depth
            }
            val newest = commits.maxBy { commit ->
                var depth = 0
                var cursor = commit
                while (cursor.parentCount > 0) {
                    cursor = walk.parseCommit(cursor.getParent(0))
                    depth++
                }
                depth
            }
            if (oldest.parentCount > 1 || newest.parentCount > 1 || commits.any { it.parentCount > 1 }) {
                throw IllegalArgumentException("Cannot squash merge commits")
            }

            val newCommitter = PersonIdent(repository)
            val mapping = HashMap<ObjectId, ObjectId>()
            repository.newObjectInserter().use { inserter ->
                val squashed = CommitBuilder().apply {
                    setTreeId(newest.tree)
                    if (oldest.parentCount > 0) setParentId(oldest.getParent(0))
                    author = oldest.authorIdent
                    committer = newCommitter
                    encoding = Charsets.UTF_8
                    this.message = message
                }
                signer?.sign(squashed, newCommitter)
                val squashedId = inserter.insert(squashed)
                commits.forEach { mapping[it.toObjectId()] = squashedId }
                mapping[oldest.toObjectId()] = squashedId

                walk.reset()
                walk.sort(RevSort.TOPO, true)
                walk.sort(RevSort.REVERSE, true)
                walk.markStart(head)
                if (oldest.parentCount > 0) walk.markUninteresting(walk.parseCommit(oldest.getParent(0)))
                for (commit in walk) {
                    if (commit.toObjectId() in ids || commit.id == oldest.id) continue
                    val oldParents = commit.parents.map { it.toObjectId() }
                    val newParents = oldParents.map { mapping[it] ?: it }
                    if (newParents == oldParents) continue
                    val builder = CommitBuilder().apply {
                        setTreeId(commit.tree)
                        setParentIds(newParents)
                        author = commit.authorIdent
                        committer = newCommitter
                        encoding = runCatching { commit.encoding }.getOrDefault(Charsets.UTF_8)
                        this.message = commit.fullMessage
                    }
                    signer?.sign(builder, newCommitter)
                    mapping[commit.toObjectId()] = inserter.insert(builder)
                }
                inserter.flush()
            }
            return Result(headId, mapping[headId] ?: headId)
        }
    }
}
