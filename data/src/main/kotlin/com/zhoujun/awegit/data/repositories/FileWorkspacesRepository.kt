package com.zhoujun.awegit.data.repositories

import com.zhoujun.awegit.common.AppDirectories
import com.zhoujun.awegit.domain.models.WorkspacesState
import com.zhoujun.awegit.domain.repositories.WorkspacesRepository
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class FileWorkspacesRepository @Inject constructor() : WorkspacesRepository {
    private val json = Json { ignoreUnknownKeys = true; prettyPrint = true }

    override fun load(): WorkspacesState? {
        val file = java.io.File(AppDirectories.configDir(), "workspaces.json")
        if (!file.exists()) return null
        return runCatching { json.decodeFromString<WorkspacesState>(file.readText()) }.getOrNull()
    }

    override fun save(state: WorkspacesState) {
        val target = java.io.File(AppDirectories.configDir(), "workspaces.json").toPath()
        val tmp = target.resolveSibling("workspaces.json.tmp")
        Files.writeString(tmp, json.encodeToString(state))
        Files.move(tmp, target, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE)
    }
}
