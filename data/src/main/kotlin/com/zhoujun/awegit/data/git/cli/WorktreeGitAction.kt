package com.zhoujun.awegit.data.git.cli

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GenericError
import com.zhoujun.awegit.domain.interfaces.IWorktreeGitAction
import com.zhoujun.awegit.domain.models.WorktreeInfo
import com.zhoujun.awegit.domain.models.WorktreeListResult
import java.io.File
import javax.inject.Inject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

class WorktreeGitAction @Inject constructor(
    private val runner: GitCliRunner,
) : IWorktreeGitAction {
    override suspend fun list(repositoryPath: String) = withContext(Dispatchers.IO) {
        if (!runner.isAvailable()) {
            return@withContext Either.Ok(WorktreeListResult(gitAvailable = false, worktrees = emptyList()))
        }
        val result = runner.run(File(repositoryPath), "worktree", "list", "--porcelain")
        if (result.exitCode != 0) {
            Either.Err(GenericError(result.stderr.ifBlank { "git worktree list failed" }))
        } else {
            Either.Ok(WorktreeListResult(gitAvailable = true, worktrees = parse(result.stdout)))
        }
    }

    override suspend fun remove(repositoryPath: String, path: String) = withContext(Dispatchers.IO) {
        if (!runner.isAvailable()) {
            return@withContext Either.Err(GenericError("Requires Git command line"))
        }
        val result = runner.run(File(repositoryPath), "worktree", "remove", path)
        if (result.exitCode != 0) {
            Either.Err(GenericError(result.stderr.ifBlank { "git worktree remove failed" }))
        } else {
            Either.Ok(Unit)
        }
    }

    private fun parse(stdout: String): List<WorktreeInfo> {
        val entries = mutableListOf<WorktreeInfo>()
        var path = ""
        var branch = ""
        fun flush() {
            if (path.isNotEmpty()) {
                entries += WorktreeInfo(path, branch.ifBlank { "detached" })
            }
            path = ""
            branch = ""
        }
        for (line in stdout.lineSequence()) {
            when {
                line.startsWith("worktree ") -> {
                    flush()
                    path = line.removePrefix("worktree ").trim()
                }
                line.startsWith("branch ") -> branch = line.removePrefix("branch ").trim().removePrefix("refs/heads/")
                line.isBlank() -> flush()
            }
        }
        flush()
        return entries
    }
}
