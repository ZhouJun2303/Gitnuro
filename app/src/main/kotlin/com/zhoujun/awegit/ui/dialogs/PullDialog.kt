package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.Icon
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.branch
import com.zhoujun.awegit.app.generated.resources.cloud
import com.zhoujun.awegit.ui.components.fork.ForkCheckboxRow
import com.zhoujun.awegit.ui.components.fork.ForkDropdown
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog
import org.jetbrains.compose.resources.painterResource

@Composable
fun PullDialog(viewModel: PullDialogViewModel, onDismiss: () -> Unit) {
    val state by viewModel.state.collectAsState()
    when (val s = state) {
        PullDialogState.Loading ->
            ForkDialog("Pull", "Loading remotes…", "Pull", {}, onDismiss, primaryEnabled = false) {}

        PullDialogState.NoRemotes ->
            ForkDialog(
                "Pull",
                "This repository has no remotes. Add one first.",
                "Pull",
                {},
                onDismiss,
                primaryEnabled = false,
            ) {}

        is PullDialogState.Loaded -> ForkDialog(
            title = "Pull",
            subtitle = "Pull remote branches and merge them into your local branch",
            primaryText = "Pull",
            primaryEnabled = s.selectedBranch != null,
            onPrimary = {
                viewModel.pull()
                onDismiss()
            },
            onDismiss = onDismiss,
        ) {
            ForkFormRow("Remote:") {
                ForkDropdown(
                    s.remotes,
                    s.selectedRemote,
                    { it.remote.name },
                    viewModel::selectRemote,
                    Modifier.weight(1f),
                    Res.drawable.cloud,
                )
            }
            ForkFormRow("Branch:") {
                ForkDropdown(
                    s.selectedRemote.branchesList,
                    s.selectedBranch,
                    { it.simpleNameWithRemote },
                    viewModel::selectBranch,
                    Modifier.weight(1f),
                    Res.drawable.branch,
                )
            }
            ForkFormRow("Into:") {
                Icon(painterResource(Res.drawable.branch), contentDescription = null, modifier = Modifier.size(14.dp))
                Spacer(Modifier.width(6.dp))
                Text(s.currentBranchName, fontSize = 12.sp)
            }
            ForkCheckboxRow("Rebase instead of merge", s.rebase, viewModel::setRebase)
            ForkCheckboxRow("Stash and reapply local changes", s.stashAndReapply, viewModel::setStashAndReapply)
        }
    }
}
