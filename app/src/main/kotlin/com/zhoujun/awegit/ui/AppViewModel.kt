package com.zhoujun.awegit.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.zhoujun.awegit.di.TabComponent
import com.zhoujun.awegit.domain.models.RepositorySelectionState
import com.zhoujun.awegit.domain.models.WorkspacesState
import com.zhoujun.awegit.domain.services.WorkspacesService
import com.zhoujun.awegit.domain.usecases.OpenPathInSystemUseCase
import com.zhoujun.awegit.domain.usecases.ScanForRepositoriesUseCase
import com.zhoujun.awegit.ui.components.TabInformation
import com.zhoujun.awegit.viewmodels.RepositoryTabViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AppViewModel @Inject constructor(
    private val tabComponentFactory: TabComponent.Factory,
    private val workspacesService: WorkspacesService,
    private val scanForRepositoriesUseCase: ScanForRepositoriesUseCase,
    private val openPathInSystemUseCase: OpenPathInSystemUseCase,
) : ViewModel() {
    val tabs: StateFlow<List<TabInformation<RepositoryTabViewModel>>>
        field = MutableStateFlow<List<TabInformation<RepositoryTabViewModel>>>(emptyList())

    val currentTab: StateFlow<TabInformation<RepositoryTabViewModel>?>
        field = MutableStateFlow<TabInformation<RepositoryTabViewModel>?>(null)

    val workspacesState: StateFlow<WorkspacesState> = workspacesService.state

    fun loadPersistedTabs() {
        val workspace = workspacesService.state.value.current
        tabs.value = workspace.openTabs.map { newAppTab2(path = it) }.ifEmpty { listOf(newAppTab2()) }
        currentTab.value = tabs.value.getOrNull(workspace.selectedTabIndex) ?: tabs.value.first()
    }

    suspend fun addNewTabFromPath(path: String, selectTab: Boolean, tabToBeReplacedPath: String? = null) {
        val tabToBeReplaced = tabs
            .value
            .firstOrNull {
                it.data.repositoryPath.firstOrNull() == tabToBeReplacedPath
            }

        val newTab = newAppTab2(
            path = path,
        )

        tabs.update {
            val newTabsList = it.toMutableList()

            if (tabToBeReplaced != null) {
                val index = newTabsList.indexOf(tabToBeReplaced)
                newTabsList[index] = newTab
            } else {
                newTabsList.add(newTab)
            }

            newTabsList
        }

        if (selectTab) {
            currentTab.value = newTab
        }
    }

    fun selectTab(tab: TabInformation<RepositoryTabViewModel>) {
        currentTab.value = tab
        viewModelScope.launch { updatePersistedTabs() }
    }

    fun closeTab(tab: TabInformation<RepositoryTabViewModel>) = viewModelScope.launch {
        val tabsList = tabs.value.toMutableList()
        var newCurrentTab: TabInformation<RepositoryTabViewModel>? = null
        tab.data.dispose()

        if (currentTab.value == tab) {
            val index = tabsList.indexOf(tab)

            if (tabsList.count() == 1) {
                newCurrentTab = newAppTab2()
            } else if (index > 0) {
                newCurrentTab = tabsList[index - 1]
            } else if (index == 0) {
                newCurrentTab = tabsList[1]
            }
        }

        tabsList.remove(tab)

        if (newCurrentTab != null) {
            if (!tabsList.contains(newCurrentTab)) {
                tabsList.add(newCurrentTab)
            }

            tabs.value = tabsList
            currentTab.value = newCurrentTab
        } else {
            tabs.value = tabsList
        }

        updatePersistedTabs()
        System.gc()
    }

    suspend fun updatePersistedTabs() {
        val open = tabs.value.filter { it.data.repositorySelectionState.value is RepositorySelectionState.Open }
        val paths = open.map { it.data.repositoryPath.firstOrNull().orEmpty() }
        workspacesService.saveOpenTabs(workspacesService.currentWorkspaceId, paths, open.indexOf(currentTab.value))
    }

    fun switchWorkspace(workspaceId: String) = viewModelScope.launch {
        if (workspaceId == workspacesService.currentWorkspaceId) return@launch
        updatePersistedTabs()
        tabs.value.forEach { it.data.dispose() }
        workspacesService.switchTo(workspaceId)
        loadPersistedTabs()
        System.gc()
    }

    fun createWorkspace(name: String) {
        switchWorkspace(workspacesService.create(name).id)
    }

    fun renameWorkspace(id: String, name: String) = workspacesService.rename(id, name)

    fun deleteWorkspace(id: String) = viewModelScope.launch {
        if (id == workspacesService.currentWorkspaceId) {
            val other = workspacesService.state.value.workspaces.firstOrNull { it.id != id } ?: return@launch
            switchWorkspace(other.id).join()
        }
        workspacesService.delete(id)
    }

    fun moveRepositoryToWorkspace(path: String, fromId: String, toId: String) = viewModelScope.launch {
        if (fromId == workspacesService.currentWorkspaceId) {
            tabs.value.filter { it.data.repositoryPath.value == path }.forEach { closeTab(it).join() }
        }
        workspacesService.moveRepository(path, fromId, toId)
    }

    fun openRepositoryInWorkspace(path: String, workspaceId: String) = viewModelScope.launch {
        if (workspaceId != workspacesService.currentWorkspaceId) switchWorkspace(workspaceId).join()
        val existing = tabs.value.firstOrNull { it.data.repositoryPath.value == path }
        if (existing != null) {
            selectTab(existing)
        } else {
            addNewTabFromPath(path, selectTab = true)
            updatePersistedTabs()
        }
    }

    fun addRepositories(workspaceId: String, paths: List<String>) = workspacesService.addRepositories(workspaceId, paths)

    fun removeRepository(workspaceId: String, path: String) = workspacesService.removeRepository(workspaceId, path)

    fun scanFolder(workspaceId: String, root: String) = viewModelScope.launch {
        workspacesService.addRepositories(workspaceId, scanForRepositoriesUseCase(root))
    }

    fun showInFileManager(path: String) {
        openPathInSystemUseCase(path)
    }

    fun addNewEmptyTab() {
        val newTab = newAppTab2()

        tabs.update {
            it.toMutableList().apply {
                add(newTab)
            }
        }

        currentTab.value = newTab
    }

    private fun newAppTab2(path: String? = null): TabInformation<RepositoryTabViewModel> {
        val tabComponent: TabComponent = tabComponentFactory.create()
        val viewModel = tabComponent
            .repositoryTabViewModelFactory()
            .create(path)

        return TabInformation(viewModel)
    }

    fun onMoveTab(fromIndex: Int, toIndex: Int) = viewModelScope.launch {
        tabs.update {
            it.toMutableList().apply {
                add(toIndex, removeAt(fromIndex))
            }
        }

        updatePersistedTabs()
    }
}
