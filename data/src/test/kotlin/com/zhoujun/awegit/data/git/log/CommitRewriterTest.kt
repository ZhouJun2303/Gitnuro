package com.zhoujun.awegit.data.git.log

import com.zhoujun.awegit.data.testutils.TestRepository
import org.eclipse.jgit.lib.Constants
import org.eclipse.jgit.revwalk.RevWalk
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows
import org.junit.jupiter.api.io.TempDir
import java.io.File

class CommitRewriterTest {
    @TempDir
    lateinit var dir: File

    @Test
    fun `rewording HEAD updates the message and moves the branch`() {
        TestRepository(dir).use { repo ->
            val first = repo.commitFile("a.txt", "1", "first")
            val head = repo.commitFile("b.txt", "2", "second")
            val oldHead = repo.repository.resolve(Constants.HEAD)
            val oldTree = head.tree

            val result = CommitRewriter.reword(repo.repository, head.name, "updated\n\nbody")
            CommitRewriter.moveHead(repo.repository, result, "reword: updated")

            RevWalk(repo.repository).use { walk ->
                val rewritten = walk.parseCommit(result.newHead)
                assertTrue(rewritten.fullMessage.startsWith("updated"))
                assertTrue(rewritten.fullMessage.contains("body"))
                assertEquals(first.toObjectId(), rewritten.getParent(0).toObjectId())
                assertEquals(oldTree, rewritten.tree)
            }
            assertEquals(result.newHead, repo.repository.resolve(Constants.HEAD))
            assertEquals(oldHead, repo.repository.resolve(Constants.ORIG_HEAD))
            assertEquals("refs/heads/main", repo.repository.fullBranch)
        }
    }

    @Test
    fun `rewording a middle commit rewrites descendants and keeps trees`() {
        TestRepository(dir).use { repo ->
            val c1 = repo.commitFile("a.txt", "1", "c1")
            val c2 = repo.commitFile("b.txt", "2", "c2")
            val c3 = repo.commitFile("c.txt", "3", "c3")

            val result = CommitRewriter.reword(repo.repository, c2.name, "c2 rewritten")
            CommitRewriter.moveHead(repo.repository, result, "reword: c2 rewritten")

            RevWalk(repo.repository).use { walk ->
                val c3New = walk.parseCommit(result.newHead)
                val c2New = walk.parseCommit(c3New.getParent(0))
                assertEquals("c3", c3New.shortMessage)
                assertEquals(c3.tree, c3New.tree)
                assertTrue(c2New.fullMessage.startsWith("c2 rewritten"))
                assertEquals(c2.tree, c2New.tree)
                assertEquals(c1.toObjectId(), c2New.getParent(0).toObjectId())
                assertEquals(c2New.toObjectId(), c3New.getParent(0).toObjectId())
            }
        }
    }

    @Test
    fun `rewording before a merge rewrites both parents of the merge`() {
        TestRepository(dir).use { repo ->
            repo.commitFile("a.txt", "1", "c1")
            val c2 = repo.commitFile("b.txt", "2", "c2")
            repo.git.checkout().setCreateBranch(true).setName("side").call()
            val side = repo.commitFile("side.txt", "s", "s1")
            repo.git.checkout().setName("main").call()
            val c3 = repo.commitFile("c.txt", "3", "c3")
            repo.git.merge().include(side).call()
            val merge = repo.repository.resolve(Constants.HEAD)

            val result = CommitRewriter.reword(repo.repository, c2.name, "c2 rewritten")
            CommitRewriter.moveHead(repo.repository, result, "reword: c2 rewritten")

            RevWalk(repo.repository).use { walk ->
                val mergeNew = walk.parseCommit(result.newHead)
                assertEquals(2, mergeNew.parentCount)
                val firstParent = walk.parseCommit(mergeNew.getParent(0))
                val secondParent = walk.parseCommit(mergeNew.getParent(1))
                assertEquals("c3", firstParent.shortMessage)
                assertEquals(c3.tree, firstParent.tree)
                assertEquals("s1", secondParent.shortMessage)
                assertEquals(side.tree, secondParent.tree)
                assertTrue(firstParent.getParent(0).name != c2.name)
                assertTrue(walk.parseCommit(merge).name != mergeNew.name)
            }
        }
    }

    @Test
    fun `reword leaves the worktree and index untouched`() {
        TestRepository(dir).use { repo ->
            val head = repo.commitFile("a.txt", "base", "base")
            repo.writeFile("a.txt", "unstaged")
            repo.writeFile("b.txt", "staged")
            repo.git.add().addFilepattern("b.txt").call()
            val indexBefore = indexSnapshot(repo)

            val result = CommitRewriter.reword(repo.repository, head.name, "rewritten")
            CommitRewriter.moveHead(repo.repository, result, "reword: rewritten")

            assertEquals("unstaged", File(dir, "a.txt").readText())
            assertEquals("staged", File(dir, "b.txt").readText())
            assertEquals(indexBefore, indexSnapshot(repo))
        }
    }

    @Test
    fun `target that is not an ancestor of HEAD is rejected`() {
        TestRepository(dir).use { repo ->
            repo.commitFile("a.txt", "1", "c1")
            repo.git.checkout().setCreateBranch(true).setName("side").call()
            val side = repo.commitFile("b.txt", "2", "side")
            repo.git.checkout().setName("main").call()

            assertThrows<IllegalArgumentException> {
                CommitRewriter.reword(repo.repository, side.name, "nope")
            }
        }
    }

    private fun indexSnapshot(repo: TestRepository): List<Pair<String, String>> {
        val cache = repo.repository.readDirCache()
        return (0 until cache.entryCount).map { i ->
            val entry = cache.getEntry(i)
            entry.pathString to entry.objectId.name
        }
    }
}
