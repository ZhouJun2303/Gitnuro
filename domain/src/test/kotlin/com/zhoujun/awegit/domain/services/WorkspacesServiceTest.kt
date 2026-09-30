package com.zhoujun.awegit.domain.services

import com.zhoujun.awegit.domain.models.DEFAULT_WORKSPACE_ID
import com.zhoujun.awegit.domain.models.Workspace
import com.zhoujun.awegit.domain.models.WorkspacesState
import com.zhoujun.awegit.domain.repositories.AppSettingsRepository
import com.zhoujun.awegit.domain.repositories.WorkspacesRepository
import io.mockk.every
import io.mockk.mockk
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WorkspacesServiceTest {
    @Test
    fun `missing file migrates tabs and recents into Default and saves once`() {
        val saved = mutableListOf<WorkspacesState>()
        val repository = object : WorkspacesRepository {
            override fun load(): WorkspacesState? = null
            override fun save(state: WorkspacesState) {
                saved += state
            }
        }
        val settings = mockk<AppSettingsRepository>()
        every { settings.latestTabsOpened } returns """["/tab"]"""
        every { settings.latestOpenedRepositoriesPath } returns """["/recent"]"""
        every { settings.latestRepositoryTabSelected } returns 0

        val service = WorkspacesService(repository, settings)

        assertEquals(1, saved.size)
        assertEquals(DEFAULT_WORKSPACE_ID, service.state.value.currentWorkspaceId)
        assertEquals(listOf("/recent", "/tab"), service.state.value.current.repositories)
        assertEquals(listOf("/tab"), service.state.value.current.openTabs)
    }

    @Test
    fun `create switch rename and delete keep a current workspace`() {
        val service = service(stored = null)
        val created = service.create("Feature")
        service.switchTo(created.id)
        service.rename(created.id, "Renamed")
        assertEquals("Renamed", service.state.value.current.name)

        service.delete(created.id)
        assertEquals(DEFAULT_WORKSPACE_ID, service.state.value.currentWorkspaceId)
        assertEquals(1, service.state.value.workspaces.size)

        val before = service.state.value
        service.delete(DEFAULT_WORKSPACE_ID)
        assertEquals(before, service.state.value)
    }

    @Test
    fun `opening a repository moves it to the front and moving removes the open tab`() {
        val stored = WorkspacesState(
            workspaces = listOf(
                Workspace(DEFAULT_WORKSPACE_ID, "Default", repositories = listOf("/b", "/a"), openTabs = listOf("/a")),
                Workspace("other", "Other"),
            ),
            currentWorkspaceId = DEFAULT_WORKSPACE_ID,
        )
        val service = service(stored)
        service.recordRepositoryOpened("/a")
        assertEquals(listOf("/a", "/b"), service.state.value.current.repositories)

        service.moveRepository("/a", DEFAULT_WORKSPACE_ID, "other")
        val source = service.state.value.workspaces.first { it.id == DEFAULT_WORKSPACE_ID }
        val target = service.state.value.workspaces.first { it.id == "other" }
        assertTrue("/a" !in source.repositories)
        assertTrue("/a" !in source.openTabs)
        assertEquals("/a", target.repositories.first())
    }

    private fun service(stored: WorkspacesState?): WorkspacesService {
        val repository = object : WorkspacesRepository {
            var value = stored
            override fun load() = value
            override fun save(state: WorkspacesState) {
                value = state
            }
        }
        val settings = mockk<AppSettingsRepository>(relaxed = true)
        every { settings.latestTabsOpened } returns ""
        every { settings.latestOpenedRepositoriesPath } returns ""
        every { settings.latestRepositoryTabSelected } returns 0
        return WorkspacesService(repository, settings)
    }
}
