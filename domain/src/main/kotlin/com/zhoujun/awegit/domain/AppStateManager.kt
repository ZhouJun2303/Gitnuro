package com.zhoujun.awegit.domain

import com.zhoujun.awegit.domain.services.WorkspacesService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AppStateManager @Inject constructor(
    private val workspacesService: WorkspacesService,
) {
    val latestOpenedRepositoriesPaths: StateFlow<List<String>> = workspacesService.currentRepositories

    val latestOpenedRepositoryPath: String
        get() = latestOpenedRepositoriesPaths.value.firstOrNull() ?: ""

    suspend fun repositoryTabChanged(path: String) = withContext(Dispatchers.IO) {
        workspacesService.recordRepositoryOpened(path)
    }

    fun loadRepositoriesTabs() = Unit

    suspend fun removeRepositoryFromRecent(path: String) = withContext(Dispatchers.IO) {
        workspacesService.removeRepository(workspacesService.currentWorkspaceId, path)
    }
}
