package com.zhoujun.awegit.ui.dialogs

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.zhoujun.awegit.domain.usecases.MergeMode
import com.zhoujun.awegit.ui.components.fork.ForkDropdown
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun MergeDialog(
    viewModel: MergeDialogViewModel,
    onDismiss: () -> Unit,
) {
    val currentBranch by viewModel.currentBranchName.collectAsState("HEAD")
    var mode by remember { mutableStateOf(MergeMode.Default) }

    ForkDialog(
        title = "Merge",
        subtitle = "Merge ${viewModel.branch.simpleNameWithRemote} into $currentBranch",
        primaryText = "Merge",
        onPrimary = {
            viewModel.merge(mode)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Type:") {
            ForkDropdown(
                items = MergeMode.entries,
                selected = mode,
                itemLabel = { it.label() },
                onSelected = { mode = it },
                modifier = Modifier.weight(1f),
            )
        }
    }
}

private fun MergeMode.label(): String = when (this) {
    MergeMode.Default -> "Default"
    MergeMode.NoFastForward -> "No fast-forward"
    MergeMode.FastForwardOnly -> "Fast-forward only"
    MergeMode.Squash -> "Squash"
}
