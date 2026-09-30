package com.zhoujun.awegit.domain.services

import com.zhoujun.awegit.domain.models.DEFAULT_WORKSPACE_ID
import com.zhoujun.awegit.domain.models.Workspace
import com.zhoujun.awegit.domain.models.WorkspacesState
import com.zhoujun.awegit.domain.repositories.AppSettingsRepository
import com.zhoujun.awegit.domain.repositories.WorkspacesRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.serialization.json.Json
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class WorkspacesService @Inject constructor(
    private val workspacesRepository: WorkspacesRepository,
    private val appSettingsRepository: AppSettingsRepository,
) {
    private val lock = Any()
    private val _state = MutableStateFlow(loadOrMigrate())
    val state: StateFlow<WorkspacesState> = _state.asStateFlow()

    private val _currentRepositories = MutableStateFlow(_state.value.current.repositories)
    val currentRepositories: StateFlow<List<String>> = _currentRepositories.asStateFlow()

    val currentWorkspaceId: String get() = _state.value.currentWorkspaceId

    private fun loadOrMigrate(): WorkspacesState {
        workspacesRepository.load()?.takeIf { it.workspaces.isNotEmpty() }?.let { return it.normalized() }
        val json = Json { ignoreUnknownKeys = true }
        val tabs = runCatching { json.decodeFromString<List<String>>(appSettingsRepository.latestTabsOpened) }.getOrDefault(emptyList())
        val recents = runCatching { json.decodeFromString<List<String>>(appSettingsRepository.latestOpenedRepositoriesPath) }.getOrDefault(emptyList())
        val default = Workspace(
            id = DEFAULT_WORKSPACE_ID,
            name = "Default",
            repositories = (recents + tabs).distinct(),
            openTabs = tabs,
            selectedTabIndex = appSettingsRepository.latestRepositoryTabSelected.coerceAtLeast(0),
        )
        return WorkspacesState(listOf(default), DEFAULT_WORKSPACE_ID).also { workspacesRepository.save(it) }
    }

    private fun WorkspacesState.normalized(): WorkspacesState {
        val list = workspaces.ifEmpty { listOf(Workspace(DEFAULT_WORKSPACE_ID, "Default")) }
            .map { it.copy(repositories = it.repositories.distinct()) }
        val currentId = if (list.any { it.id == currentWorkspaceId }) currentWorkspaceId else list.first().id
        return WorkspacesState(list, currentId)
    }

    private fun mutate(transform: (WorkspacesState) -> WorkspacesState) = synchronized(lock) {
        val newState = transform(_state.value).normalized()
        _state.value = newState
        _currentRepositories.value = newState.current.repositories
        workspacesRepository.save(newState)
    }

    private fun updateWorkspace(id: String, transform: (Workspace) -> Workspace) =
        mutate { s -> s.copy(workspaces = s.workspaces.map { if (it.id == id) transform(it) else it }) }

    fun create(name: String): Workspace {
        val ws = Workspace(UUID.randomUUID().toString(), name.trim().ifEmpty { "Workspace" })
        mutate { it.copy(workspaces = it.workspaces + ws) }
        return ws
    }

    fun rename(id: String, name: String) = updateWorkspace(id) { it.copy(name = name.trim().ifEmpty { it.name }) }

    fun delete(id: String) = mutate { s ->
        if (s.workspaces.size <= 1) s else s.copy(workspaces = s.workspaces.filterNot { it.id == id })
    }

    fun switchTo(id: String) = mutate { it.copy(currentWorkspaceId = id) }

    fun recordRepositoryOpened(path: String) =
        updateWorkspace(currentWorkspaceId) { it.copy(repositories = listOf(path) + (it.repositories - path)) }

    fun removeRepository(workspaceId: String, path: String) =
        updateWorkspace(workspaceId) { it.copy(repositories = it.repositories - path) }

    fun addRepositories(workspaceId: String, paths: List<String>) =
        updateWorkspace(workspaceId) { ws -> ws.copy(repositories = ws.repositories + paths.filterNot { it in ws.repositories }) }

    fun moveRepository(path: String, fromId: String, toId: String) = mutate { s ->
        s.copy(workspaces = s.workspaces.map { ws ->
            when (ws.id) {
                fromId -> ws.copy(repositories = ws.repositories - path, openTabs = ws.openTabs - path)
                toId -> ws.copy(repositories = listOf(path) + (ws.repositories - path))
                else -> ws
            }
        })
    }

    fun saveOpenTabs(workspaceId: String, paths: List<String>, selectedIndex: Int) =
        updateWorkspace(workspaceId) { it.copy(openTabs = paths, selectedTabIndex = selectedIndex.coerceAtLeast(0)) }
}
