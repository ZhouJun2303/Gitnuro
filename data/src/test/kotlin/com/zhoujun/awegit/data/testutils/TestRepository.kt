package com.zhoujun.awegit.data.testutils

import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.Repository
import org.eclipse.jgit.revwalk.RevCommit
import java.io.File

class TestRepository(val dir: File) : AutoCloseable {
    val git: Git = Git.init().setDirectory(dir).setInitialBranch("main").call()
    val repository: Repository get() = git.repository

    init {
        repository.config.apply {
            setString("user", null, "name", "Test")
            setString("user", null, "email", "test@example.com")
            setBoolean("commit", null, "gpgsign", false)
            save()
        }
    }

    fun writeFile(path: String, content: String) =
        File(dir, path).apply { parentFile.mkdirs(); writeText(content) }

    fun commitFile(path: String, content: String, message: String): RevCommit {
        writeFile(path, content)
        git.add().addFilepattern(path).call()
        return git.commit().setMessage(message).setSign(false).call()
    }

    override fun close() = git.close()
}
