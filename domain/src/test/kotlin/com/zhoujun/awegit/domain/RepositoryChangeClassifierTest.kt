package com.zhoujun.awegit.domain

import com.zhoujun.awegit.domain.usecases.DataToRefresh
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class RepositoryChangeClassifierTest {
    private val gitDir = "D:\\r\\.git"
    private val worktree = "D:\\r"

    @Test
    fun `commit edit message is ignored`() {
        val result = RepositoryChangeClassifier.classify(listOf("D:\\r\\.git\\COMMIT_EDITMSG"), gitDir, worktree)
        assertTrue(result.dataToRefresh.isEmpty())
    }

    @Test
    fun `probe files are ignored`() {
        val result = RepositoryChangeClassifier.classify(listOf("D:\\r\\.git\\.probe-123"), gitDir, worktree)
        assertTrue(result.dataToRefresh.isEmpty())
    }

    @Test
    fun `index refreshes status`() {
        val result = RepositoryChangeClassifier.classify(listOf("D:\\r\\.git\\index"), gitDir, worktree)
        assertEquals(setOf(DataToRefresh.STATUS), result.dataToRefresh)
    }

    @Test
    fun `branch ref refreshes branches and log`() {
        val result = RepositoryChangeClassifier.classify(listOf("D:\\r\\.git\\refs\\heads\\main"), gitDir, worktree)
        assertEquals(setOf(DataToRefresh.BRANCHES, DataToRefresh.LOG), result.dataToRefresh)
    }

    @Test
    fun `objects are ignored`() {
        val result = RepositoryChangeClassifier.classify(listOf("D:\\r\\.git\\objects\\ab\\cd"), gitDir, worktree)
        assertTrue(result.dataToRefresh.isEmpty())
    }

    @Test
    fun `worktree file refreshes status`() {
        val result = RepositoryChangeClassifier.classify(listOf("D:\\r\\src\\App.kt"), gitDir, worktree)
        assertEquals(setOf(DataToRefresh.STATUS), result.dataToRefresh)
        assertEquals(listOf("src/App.kt"), result.worktreePaths)
    }

    @Test
    fun `unknown git file refreshes everything`() {
        val result = RepositoryChangeClassifier.classify(listOf("D:\\r\\.git\\unknown-file"), gitDir, worktree)
        assertEquals(setOf(DataToRefresh.ALL), result.dataToRefresh)
    }
}
