package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.ui.components.fork.ForkCheckboxRow
import com.zhoujun.awegit.ui.components.fork.ForkDropdown
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun PushDialog(viewModel: PushDialogViewModel, onDismiss: () -> Unit) {
    val localBranches by viewModel.localBranches.collectAsState(emptyList())
    val remoteNames by viewModel.remoteNames.collectAsState(emptyList())
    var branch by remember { mutableStateOf<Branch?>(null) }
    var remoteName by remember { mutableStateOf<String?>(null) }
    var remoteBranch by remember { mutableStateOf(TextFieldValue("")) }
    var branchNameEdited by remember { mutableStateOf(false) }
    var pushAllTags by remember { mutableStateOf(false) }
    var createTracking by remember { mutableStateOf(true) }
    var forcePush by remember { mutableStateOf(false) }
    val selectedLocal = branch ?: localBranches.firstOrNull()
    val selectedRemote = remoteName ?: remoteNames.firstOrNull()

    LaunchedEffect(selectedLocal?.name) {
        if (!branchNameEdited) {
            remoteBranch = TextFieldValue(selectedLocal?.simpleName.orEmpty())
        }
    }

    ForkDialog(
        title = "Push",
        subtitle = "Push local branch to remote",
        primaryText = "Push",
        primaryEnabled = selectedLocal != null && !selectedRemote.isNullOrBlank() && remoteBranch.text.isNotBlank(),
        onPrimary = {
            viewModel.push(
                force = forcePush,
                pushTags = pushAllTags,
                source = selectedLocal,
                remoteName = selectedRemote,
                remoteBranchName = remoteBranch.text,
                setUpstream = createTracking,
            )
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Branch:") {
            ForkDropdown(
                items = localBranches,
                selected = selectedLocal,
                itemLabel = { it.simpleName },
                onSelected = { branch = it },
                modifier = Modifier.weight(1f),
            )
        }
        ForkFormRow("To:") {
            ForkDropdown(
                items = remoteNames,
                selected = selectedRemote,
                itemLabel = { it },
                onSelected = { remoteName = it },
                modifier = Modifier.weight(0.4f),
            )
            Spacer(Modifier.width(8.dp))
            ForkTextField(
                value = remoteBranch,
                onValueChange = {
                    branchNameEdited = true
                    remoteBranch = it
                },
                modifier = Modifier.weight(0.6f),
            )
        }
        ForkCheckboxRow("Push all tags", pushAllTags) { pushAllTags = it }
        ForkCheckboxRow("Create tracking reference", createTracking) { createTracking = it }
        ForkCheckboxRow("Force push", forcePush) { forcePush = it }
    }
}
