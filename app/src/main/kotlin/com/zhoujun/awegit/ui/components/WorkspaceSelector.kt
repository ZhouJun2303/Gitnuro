package com.zhoujun.awegit.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.Divider
import androidx.compose.material.DropdownMenu
import androidx.compose.material.DropdownMenuItem
import androidx.compose.material.Icon
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.expand_more
import com.zhoujun.awegit.app.generated.resources.folder
import com.zhoujun.awegit.domain.models.Workspace
import com.zhoujun.awegit.domain.models.WorkspacesState
import com.zhoujun.awegit.theme.onBackgroundSecondary
import org.jetbrains.compose.resources.painterResource

@Composable
fun WorkspaceSelector(
    state: WorkspacesState,
    onSelect: (String) -> Unit,
    onCreate: () -> Unit,
    onRename: (Workspace) -> Unit,
    onDelete: (Workspace) -> Unit,
    onManage: () -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    val current = state.current
    Row(
        modifier = Modifier.height(36.dp).clickable { expanded = true }.padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(painterResource(Res.drawable.folder), contentDescription = null, modifier = Modifier.size(16.dp))
        Spacer(Modifier.width(6.dp))
        Text(current.name, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = MaterialTheme.colors.onBackground)
        Icon(
            painterResource(Res.drawable.expand_more),
            contentDescription = null,
            modifier = Modifier.size(16.dp),
            tint = MaterialTheme.colors.onBackgroundSecondary,
        )
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            state.workspaces.forEach { workspace ->
                DropdownMenuItem(onClick = {
                    expanded = false
                    onSelect(workspace.id)
                }) {
                    val mark = if (workspace.id == state.currentWorkspaceId) "✓ " else "  "
                    Text("$mark${workspace.name}  ${workspace.repositories.size}", fontSize = 12.sp)
                }
            }
            Divider()
            DropdownMenuItem(onClick = { expanded = false; onCreate() }) { Text("New Workspace…", fontSize = 12.sp) }
            DropdownMenuItem(onClick = { expanded = false; onRename(current) }) { Text("Rename Workspace…", fontSize = 12.sp) }
            DropdownMenuItem(
                onClick = { expanded = false; onDelete(current) },
                enabled = state.workspaces.size > 1,
            ) { Text("Delete Workspace…", fontSize = 12.sp) }
            Divider()
            DropdownMenuItem(onClick = { expanded = false; onManage() }) { Text("Repository Manager…", fontSize = 12.sp) }
        }
    }
}
