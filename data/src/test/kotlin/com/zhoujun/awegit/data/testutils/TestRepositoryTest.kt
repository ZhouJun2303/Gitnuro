package com.zhoujun.awegit.data.testutils

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir
import java.io.File

class TestRepositoryTest {
    @TempDir
    lateinit var dir: File

    @Test
    fun `commits twice`() {
        TestRepository(dir).use { repo ->
            repo.commitFile("a.txt", "one", "first")
            repo.commitFile("b.txt", "two", "second")
            assertEquals(2, repo.git.log().call().count())
        }
    }
}
