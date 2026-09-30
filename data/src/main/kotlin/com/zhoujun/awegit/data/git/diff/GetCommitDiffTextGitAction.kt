package com.zhoujun.awegit.data.git.diff

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IGetCommitDiffTextGitAction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.lib.ObjectId
import org.eclipse.jgit.revwalk.RevWalk
import org.eclipse.jgit.treewalk.CanonicalTreeParser
import org.eclipse.jgit.treewalk.EmptyTreeIterator
import javax.inject.Inject

class GetCommitDiffTextGitAction @Inject constructor(
    private val jgit: JGit,
) : IGetCommitDiffTextGitAction {
    override suspend fun invoke(repositoryPath: String, commitHash: String, maxChars: Int) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            val repo = git.repository
            val entries = RevWalk(repo).use { walk ->
                val commit = walk.parseCommit(ObjectId.fromString(commitHash))
                val newTree = repo.newObjectReader().use { reader ->
                    CanonicalTreeParser().apply { reset(reader, commit.tree) }
                }
                val oldTree = if (commit.parentCount > 0) {
                    repo.newObjectReader().use { reader ->
                        CanonicalTreeParser().apply { reset(reader, walk.parseCommit(commit.getParent(0)).tree) }
                    }
                } else {
                    EmptyTreeIterator()
                }
                git.diff().setOldTree(oldTree).setNewTree(newTree).setShowNameAndStatusOnly(true).call()
            }
            DiffTextBuilder.build(repo, entries, maxChars)
        }
    }
}
