package com.zhoujun.awegit.data.git.diff

import com.zhoujun.awegit.data.testutils.TestRepository
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.treewalk.EmptyTreeIterator
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir
import java.io.File

class DiffTextBuilderTest {
    @TempDir
    lateinit var dir: File

    @Test
    fun `skips lockfile contents but keeps the file in the list`() {
        TestRepository(dir).use { repo ->
            repo.commitFile("a.txt", "base", "init")
            repo.writeFile("a.txt", "changed a")
            repo.writeFile("b.txt", "new b")
            repo.writeFile("package-lock.json", "{\"lock\":true}")
            repo.git.add().addFilepattern(".").call()
            val text = stagedDiff(repo, 100_000)
            assertTrue(text.files.any { it.contains("package-lock.json") })
            assertFalse(text.text.contains("lock"))
            assertTrue(text.text.contains("changed a"))
            assertFalse(text.truncated)
        }
    }

    @Test
    fun `truncates when the budget is small`() {
        TestRepository(dir).use { repo ->
            repo.commitFile("a.txt", "base", "init")
            repo.writeFile("a.txt", "x".repeat(4_000))
            repo.writeFile("b.txt", "y".repeat(4_000))
            repo.git.add().addFilepattern(".").call()
            val text = stagedDiff(repo, 1_000)
            assertTrue(text.truncated)
            assertTrue(text.text.length <= 1_000)
        }
    }

    @Test
    fun `first commit without HEAD still produces a diff`() {
        Git.init().setDirectory(dir).setInitialBranch("main").call().use { git ->
            git.repository.config.apply {
                setString("user", null, "name", "Test")
                setString("user", null, "email", "test@example.com")
                save()
            }
            File(dir, "a.txt").writeText("hello")
            git.add().addFilepattern("a.txt").call()
            val headTree = git.repository.resolve("HEAD^{tree}")
            val entries = git.diff().setCached(true).setShowNameAndStatusOnly(true)
                .apply { if (headTree == null) setOldTree(EmptyTreeIterator()) }
                .call()
            val text = DiffTextBuilder.build(git.repository, entries, 10_000)
            assertTrue(text.files.any { it.contains("a.txt") })
            assertTrue(text.text.contains("hello"))
        }
    }

    private fun stagedDiff(repo: TestRepository, maxChars: Int): com.zhoujun.awegit.domain.models.DiffText {
        val headTree = repo.repository.resolve("HEAD^{tree}")
        val entries = repo.git.diff().setCached(true).setShowNameAndStatusOnly(true)
            .apply { if (headTree == null) setOldTree(EmptyTreeIterator()) }
            .call()
        return DiffTextBuilder.build(repo.repository, entries, maxChars)
    }
}
