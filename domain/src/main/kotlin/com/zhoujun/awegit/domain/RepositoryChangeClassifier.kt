package com.zhoujun.awegit.domain

import com.zhoujun.awegit.domain.usecases.DataToRefresh
import java.nio.file.Paths

data class ChangeClassification(
    val dataToRefresh: Set<DataToRefresh>,
    val worktreePaths: List<String>,
)

object RepositoryChangeClassifier {
    private val IGNORED_ROOT_FILES = setOf(
        "COMMIT_EDITMSG",
        "MERGE_MSG",
        "SQUASH_MSG",
        "FETCH_HEAD",
        "ORIG_HEAD",
        "AUTO_MERGE",
        "gc.pid",
        "gc.log",
        "description",
    )
    private val STATE_ROOT_FILES = setOf("MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "REBASE_HEAD", "BISECT_LOG")
    private val IGNORED_DIRS = setOf("objects", "logs", "hooks", "lfs")
    private val STATE_DIRS = setOf("rebase-merge", "rebase-apply", "sequencer")

    fun classify(paths: Collection<String>, gitDir: String, worktreeDir: String): ChangeClassification {
        val git = Paths.get(gitDir).normalize()
        val work = Paths.get(worktreeDir).normalize()
        val result = mutableSetOf<DataToRefresh>()
        val worktreePaths = mutableListOf<String>()

        for (raw in paths) {
            val p = Paths.get(raw).normalize()
            val name = p.fileName?.toString() ?: continue
            if (p.startsWith(git)) {
                if (p == git || name.startsWith(".probe-") || name.endsWith(".lock")) continue
                val rel = git.relativize(p)
                val first = rel.getName(0).toString()
                val atRoot = rel.nameCount == 1
                when {
                    atRoot && first in IGNORED_ROOT_FILES -> Unit
                    first in IGNORED_DIRS -> Unit
                    atRoot && first == "index" -> result += DataToRefresh.STATUS
                    atRoot && first == "HEAD" -> result += listOf(
                        DataToRefresh.BRANCHES,
                        DataToRefresh.LOG,
                        DataToRefresh.STATUS,
                        DataToRefresh.REPO_STATE,
                    )
                    atRoot && first == "packed-refs" -> result += listOf(
                        DataToRefresh.BRANCHES,
                        DataToRefresh.REMOTES,
                        DataToRefresh.TAGS,
                        DataToRefresh.LOG,
                    )
                    atRoot && first == "config" -> result += listOf(
                        DataToRefresh.GIT_CONFIG,
                        DataToRefresh.REMOTES,
                        DataToRefresh.BRANCHES,
                    )
                    atRoot && first in STATE_ROOT_FILES -> result += listOf(
                        DataToRefresh.REPO_STATE,
                        DataToRefresh.STATUS,
                        DataToRefresh.LOG,
                    )
                    first in STATE_DIRS -> result += listOf(DataToRefresh.REPO_STATE, DataToRefresh.STATUS, DataToRefresh.LOG)
                    first == "info" -> result += DataToRefresh.STATUS
                    first == "modules" -> result += listOf(DataToRefresh.SUBMODULES, DataToRefresh.STATUS)
                    first == "worktrees" -> Unit
                    first == "refs" -> {
                        val second = if (rel.nameCount > 1) rel.getName(1).toString() else ""
                        result += when (second) {
                            "heads" -> listOf(DataToRefresh.BRANCHES, DataToRefresh.LOG)
                            "remotes" -> listOf(DataToRefresh.REMOTES, DataToRefresh.BRANCHES, DataToRefresh.LOG)
                            "tags" -> listOf(DataToRefresh.TAGS, DataToRefresh.LOG)
                            "stash" -> listOf(DataToRefresh.STASHES, DataToRefresh.LOG)
                            else -> listOf(DataToRefresh.LOG)
                        }
                    }
                    else -> result += DataToRefresh.ALL
                }
            } else if (p.startsWith(work)) {
                worktreePaths += work.relativize(p).toString().replace('\\', '/')
                result += DataToRefresh.STATUS
                if (name == ".gitmodules") result += DataToRefresh.SUBMODULES
            }
        }
        return ChangeClassification(result, worktreePaths)
    }
}
