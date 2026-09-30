package com.zhoujun.awegit.ui.dialogs

import androidx.compose.desktop.ui.tooling.preview.Preview
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Modifier
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.RemoteInfo
import com.zhoujun.awegit.ui.components.fork.ForkDropdown
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog
import com.zhoujun.awegit.viewmodels.SetDefaultUpstreamBranchState
import com.zhoujun.awegit.viewmodels.SetUpstreamBranchDialogViewModel

@Preview
@Composable
fun SetDefaultUpstreamBranchDialogPreview() {
    SetDefaultUpstreamBranchDialogView(
        state = SetDefaultUpstreamBranchState.Loaded(
            Branch("XYZ", "main", isLocal = true),
            null,
            emptyList(),
            null,
            null,
            isCompleted = false,
        ),
        onDismiss = {},
        setSelectedRemote = {},
        setSelectedBranch = {},
        changeDefaultUpstreamBranch = {},
    )
}

@Composable
fun SetDefaultUpstreamBranchDialog(
    viewModel: SetUpstreamBranchDialogViewModel,
    onDismiss: () -> Unit,
) {
    val state = viewModel.setDefaultUpstreamBranchState.collectAsState().value
    LaunchedEffect(state) {
        if (state is SetDefaultUpstreamBranchState.Loaded && state.isCompleted) {
            onDismiss()
        }
    }

    SetDefaultUpstreamBranchDialogView(
        state = state,
        onDismiss = onDismiss,
        setSelectedRemote = { viewModel.setSelectedRemote(it) },
        setSelectedBranch = { viewModel.setSelectedBranch(it) },
        changeDefaultUpstreamBranch = { viewModel.changeDefaultUpstreamBranch() },
    )
}

@Composable
private fun SetDefaultUpstreamBranchDialogView(
    state: SetDefaultUpstreamBranchState,
    onDismiss: () -> Unit,
    setSelectedRemote: (RemoteInfo) -> Unit,
    setSelectedBranch: (Branch) -> Unit,
    changeDefaultUpstreamBranch: () -> Unit,
) {
    val loaded = state as? SetDefaultUpstreamBranchState.Loaded
    ForkDialog(
        title = "Set Upstream Branch",
        subtitle = null,
        primaryText = "Set Upstream",
        primaryEnabled = loaded?.selectedRemote != null && loaded.selectedBranch != null,
        onPrimary = changeDefaultUpstreamBranch,
        onDismiss = onDismiss,
    ) {
        if (loaded != null) {
            ForkFormRow("Remote:") {
                ForkDropdown(
                    items = loaded.remotes,
                    selected = loaded.selectedRemote,
                    itemLabel = { it.remote.name },
                    onSelected = setSelectedRemote,
                    modifier = Modifier.weight(1f),
                )
            }
            ForkFormRow("Branch:") {
                ForkDropdown(
                    items = loaded.selectedRemote?.branchesList.orEmpty(),
                    selected = loaded.selectedBranch,
                    itemLabel = { it.simpleName },
                    onSelected = setSelectedBranch,
                    modifier = Modifier.weight(1f),
                )
            }
        }
    }
}
