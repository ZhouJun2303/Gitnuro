package com.zhoujun.awegit.data.git.workspace

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.conflicts.ConflictSides
import com.zhoujun.awegit.domain.errors.GenericError
import com.zhoujun.awegit.domain.errors.raiseError
import com.zhoujun.awegit.domain.interfaces.IResolveConflictGitAction
import java.io.File
import javax.inject.Inject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.dircache.DirCacheEntry

class ResolveConflictGitAction @Inject constructor(
    private val jgit: JGit,
) : IResolveConflictGitAction {
    override suspend fun read(repositoryPath: String, path: String) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            val wanted = normalize(path)
            var mine = ""
            var theirs = ""
            val cache = git.repository.readDirCache()
            for (index in 0 until cache.entryCount) {
                val entry = cache.getEntry(index)
                if (normalize(entry.pathString) != wanted) continue
                val text = git.repository.open(entry.objectId).openStream().bufferedReader().use { it.readText() }
                when (entry.stage) {
                    DirCacheEntry.STAGE_2 -> mine = text
                    DirCacheEntry.STAGE_3 -> theirs = text
                }
            }
            val working = File(git.repository.workTree, path).takeIf { it.isFile }?.readText().orEmpty()
            ConflictSides(mine = mine, theirs = theirs, workingTree = working)
        }
    }

    override suspend fun useStage(repositoryPath: String, path: String, stage: Int) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            val wanted = normalize(path)
            val cache = git.repository.readDirCache()
            val entry = (0 until cache.entryCount)
                .map { cache.getEntry(it) }
                .firstOrNull { normalize(it.pathString) == wanted && it.stage == stage }
                ?: raiseError(GenericError("Stage $stage was not found for $path"))
            val file = File(git.repository.workTree, path)
            file.parentFile?.mkdirs()
            git.repository.open(entry.objectId).openStream().use { input ->
                file.outputStream().use { output -> input.copyTo(output) }
            }
            git.add().addFilepattern(wanted).call()
            Unit
        }
    }

    override suspend fun markResolved(repositoryPath: String, path: String) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            git.add().addFilepattern(normalize(path)).call()
            Unit
        }
    }

    private fun normalize(path: String) = path.replace('\\', '/')
}
