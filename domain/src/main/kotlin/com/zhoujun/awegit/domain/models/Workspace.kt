package com.zhoujun.awegit.domain.models

import kotlinx.serialization.Serializable

const val DEFAULT_WORKSPACE_ID = "default"

@Serializable
data class Workspace(
    val id: String,
    val name: String,
    val repositories: List<String> = emptyList(),
    val openTabs: List<String> = emptyList(),
    val selectedTabIndex: Int = 0,
)

@Serializable
data class WorkspacesState(
    val workspaces: List<Workspace>,
    val currentWorkspaceId: String,
) {
    val current: Workspace
        get() = workspaces.firstOrNull { it.id == currentWorkspaceId } ?: workspaces.first()
}
