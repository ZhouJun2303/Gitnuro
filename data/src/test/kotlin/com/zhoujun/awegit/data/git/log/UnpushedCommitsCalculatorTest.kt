package com.zhoujun.awegit.data.git.log

import com.zhoujun.awegit.data.testutils.TestRepository
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.Constants
import org.eclipse.jgit.transport.RefSpec
import org.eclipse.jgit.transport.URIish
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows
import org.junit.jupiter.api.io.TempDir
import java.io.File

class UnpushedCommitsCalculatorTest {
    @TempDir
    lateinit var dir: File

    @Test
    fun `without a remote every commit is unpushed`() {
        TestRepository(File(dir, "work")).use { repo ->
            val first = repo.commitFile("a.txt", "1", "one")
            val second = repo.commitFile("b.txt", "2", "two")
            assertEquals(setOf(first.name, second.name), UnpushedCommitsCalculator.compute(repo.repository))
            assertEquals(1, UnpushedCommitsCalculator.compute(repo.repository, limit = 1).size)
        }
    }

    @Test
    fun `after push only the new commits are unpushed`() {
        val bareDir = File(dir, "bare")
        Git.init().setBare(true).setDirectory(bareDir).call().use {
            TestRepository(File(dir, "work")).use { repo ->
                repo.commitFile("a.txt", "1", "one")
                repo.git.remoteAdd().setName("origin").setUri(URIish(bareDir.absolutePath)).call()
                repo.git.push()
                    .setRemote("origin")
                    .setRefSpecs(RefSpec("refs/heads/main:refs/heads/main"))
                    .call()
                val second = repo.commitFile("b.txt", "2", "two")
                val third = repo.commitFile("c.txt", "3", "three")
                assertEquals(setOf(second.name, third.name), UnpushedCommitsCalculator.compute(repo.repository))
            }
        }
    }
}
