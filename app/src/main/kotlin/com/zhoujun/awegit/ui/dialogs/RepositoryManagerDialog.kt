package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.material.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.pointer.isCtrlPressed
import androidx.compose.ui.input.pointer.isShiftPressed
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.domain.models.Workspace
import com.zhoujun.awegit.theme.backgroundSelected
import com.zhoujun.awegit.theme.onBackgroundSecondary
import com.zhoujun.awegit.ui.AppViewModel
import com.zhoujun.awegit.ui.context_menu.ContextMenu
import com.zhoujun.awegit.ui.context_menu.ContextMenuElement
import com.zhoujun.awegit.ui.dialogs.base.MaterialDialog
import java.io.File

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun RepositoryManagerDialog(
    appViewModel: AppViewModel,
    onOpenDirectory: () -> String?,
    onRenameWorkspace: (Workspace) -> Unit,
    onDismiss: () -> Unit,
) {
    val state by appViewModel.workspacesState.collectAsState()
    var selectedWorkspaceId by remember(state.currentWorkspaceId) { mutableStateOf(state.currentWorkspaceId) }
    var query by remember { mutableStateOf("") }
    var selectedPaths by remember { mutableStateOf(setOf<String>()) }
    val workspace = state.workspaces.firstOrNull { it.id == selectedWorkspaceId } ?: state.current
    val repositories = workspace.repositories.filter { path ->
        val name = File(path).name
        query.isBlank() || name.contains(query, ignoreCase = true) || path.contains(query, ignoreCase = true)
    }

    MaterialDialog(paddingHorizontal = 0.dp, paddingVertical = 0.dp, onCloseRequested = onDismiss) {
        Column(Modifier.width(760.dp).height(520.dp)) {
            Text(
                "Repository Manager",
                modifier = Modifier.padding(16.dp),
                fontSize = 16.sp,
                fontWeight = FontWeight.SemiBold,
            )
            Row(Modifier.weight(1f)) {
                Column(Modifier.width(200.dp).fillMaxHeight()) {
                    LazyColumn(Modifier.weight(1f)) {
                        items(state.workspaces, key = { it.id }) { item ->
                            val selected = item.id == workspace.id
                            ContextMenu(
                                items = {
                                    listOf(
                                        ContextMenuElement.ContextTextEntry("Rename") {
                                            onRenameWorkspace(item)
                                        },
                                        ContextMenuElement.ContextTextEntry(
                                            label = "Delete",
                                            onClick = { appViewModel.deleteWorkspace(item.id) },
                                        ),
                                    )
                                },
                            ) {
                                Row(
                                    Modifier
                                        .fillMaxWidth()
                                        .height(32.dp)
                                        .background(if (selected) MaterialTheme.colors.backgroundSelected else MaterialTheme.colors.background)
                                        .clickable {
                                            selectedWorkspaceId = item.id
                                            selectedPaths = emptySet()
                                        }
                                        .padding(horizontal = 12.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Text("${item.name}  ${item.repositories.size}", fontSize = 12.sp)
                                }
                            }
                        }
                    }
                    Text(
                        "+",
                        modifier = Modifier
                            .padding(12.dp)
                            .clickable { appViewModel.createWorkspace("Workspace") },
                        fontSize = 16.sp,
                    )
                }
                Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        TextField(
                            value = query,
                            onValueChange = { query = it },
                            modifier = Modifier.weight(1f),
                            placeholder = { Text("Search") },
                            singleLine = true,
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(
                            "Add Repository…",
                            modifier = Modifier.clickable {
                                val path = onOpenDirectory() ?: return@clickable
                                if (File(path, ".git").exists()) appViewModel.addRepositories(workspace.id, listOf(path))
                            },
                            fontSize = 12.sp,
                            color = MaterialTheme.colors.primary,
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(
                            "Scan Folder…",
                            modifier = Modifier.clickable {
                                val path = onOpenDirectory() ?: return@clickable
                                appViewModel.scanFolder(workspace.id, path)
                            },
                            fontSize = 12.sp,
                            color = MaterialTheme.colors.primary,
                        )
                    }
                    LazyColumn(
                        Modifier
                            .fillMaxSize()
                            .onPreviewKeyEvent { event ->
                                when (event.key) {
                                    Key.Enter -> {
                                        selectedPaths.firstOrNull()?.let {
                                            appViewModel.openRepositoryInWorkspace(it, workspace.id)
                                            onDismiss()
                                        }
                                        true
                                    }
                                    Key.Delete, Key.Backspace -> {
                                        selectedPaths.forEach { appViewModel.removeRepository(workspace.id, it) }
                                        selectedPaths = emptySet()
                                        true
                                    }
                                    else -> false
                                }
                            },
                    ) {
                        items(repositories, key = { it }) { path ->
                            val missing = !File(path).exists()
                            val selected = path in selectedPaths
                            val keyboardModifiers = LocalWindowInfo.current.keyboardModifiers
                            ContextMenu(
                                items = {
                                    val targets = if (path in selectedPaths && selectedPaths.size > 1) selectedPaths else setOf(path)
                                    buildList {
                                        add(ContextMenuElement.ContextTextEntry("Open") {
                                            appViewModel.openRepositoryInWorkspace(path, workspace.id)
                                            onDismiss()
                                        })
                                        state.workspaces.filter { it.id != workspace.id }.forEach { target ->
                                            add(ContextMenuElement.ContextTextEntry("Move to ${target.name}") {
                                                targets.forEach { item ->
                                                    appViewModel.moveRepositoryToWorkspace(item, workspace.id, target.id)
                                                }
                                            })
                                        }
                                        add(ContextMenuElement.ContextTextEntry("Remove from Workspace") {
                                            targets.forEach { appViewModel.removeRepository(workspace.id, it) }
                                        })
                                        add(ContextMenuElement.ContextTextEntry("Show in File Manager") {
                                            appViewModel.showInFileManager(path)
                                        })
                                    }
                                },
                            ) {
                                Column(
                                    Modifier
                                        .fillMaxWidth()
                                        .height(32.dp)
                                        .background(if (selected) MaterialTheme.colors.backgroundSelected else MaterialTheme.colors.background)
                                        .combinedClickable(
                                            onClick = {
                                                val ctrl = keyboardModifiers.isCtrlPressed
                                                val shift = keyboardModifiers.isShiftPressed
                                                selectedPaths = when {
                                                    ctrl -> if (path in selectedPaths) selectedPaths - path else selectedPaths + path
                                                    shift -> {
                                                        val start = repositories.indexOf(selectedPaths.lastOrNull()).coerceAtLeast(0)
                                                        val end = repositories.indexOf(path)
                                                        val range = if (start <= end) start..end else end..start
                                                        repositories.slice(range).toSet()
                                                    }
                                                    else -> setOf(path)
                                                }
                                            },
                                            onDoubleClick = {
                                                appViewModel.openRepositoryInWorkspace(path, workspace.id)
                                                onDismiss()
                                            },
                                        )
                                        .padding(horizontal = 8.dp),
                                ) {
                                    Text(
                                        File(path).name + if (missing) " (missing)" else "",
                                        fontSize = 12.sp,
                                        fontWeight = FontWeight.SemiBold,
                                        maxLines = 1,
                                    )
                                    Text(path, fontSize = 11.sp, color = MaterialTheme.colors.onBackgroundSecondary, maxLines = 1)
                                }
                            }
                        }
                    }
                }
            }
            Box(Modifier.fillMaxWidth().padding(8.dp), contentAlignment = Alignment.CenterEnd) {
                Text("Close", modifier = Modifier.clickable(onClick = onDismiss), color = MaterialTheme.colors.primary)
            }
        }
    }
}
