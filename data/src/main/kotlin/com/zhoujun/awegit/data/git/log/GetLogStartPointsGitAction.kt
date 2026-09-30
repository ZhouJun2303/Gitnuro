package com.zhoujun.awegit.data.git.log

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IGetLogStartPointsGitAction
import com.zhoujun.awegit.domain.models.LogStartPoints
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.lib.Constants
import javax.inject.Inject

class GetLogStartPointsGitAction @Inject constructor(
    private val jgit: JGit,
) : IGetLogStartPointsGitAction {
    override suspend fun invoke(repositoryPath: String) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            val repo = git.repository
            val refDb = repo.refDatabase
            val hashes = LinkedHashSet<String>()
            val refs = refDb.getRefsByPrefix(Constants.R_HEADS) +
                refDb.getRefsByPrefix(Constants.R_REMOTES) +
                refDb.getRefsByPrefix(Constants.R_TAGS)
            val hidden = runCatching {
                val configFile = java.io.File(repo.directory, "awegit")
                if (!configFile.exists()) emptySet()
                else {
                    val config = org.eclipse.jgit.storage.file.FileBasedConfig(configFile, repo.fs)
                    config.load()
                    config.getStringList("awegit", null, "hiddenRef").toSet()
                }
            }.getOrDefault(emptySet())
            for (ref in refs) {
                if (ref.name in hidden) continue
                val peeled = refDb.peel(ref)
                (peeled.peeledObjectId ?: peeled.objectId)?.let { hashes += it.name }
            }
            val stashes = runCatching { git.stashList().call().map { it.name } }.getOrDefault(emptyList())
            hashes += stashes
            LogStartPoints(hashes.toList(), stashes.toSet(), repo.resolve(Constants.HEAD)?.name)
        }
    }
}
