package com.zhoujun.awegit.ui.dialogs

import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.RemoteInfo
import com.zhoujun.awegit.domain.models.Tag
import com.zhoujun.awegit.ui.components.fork.ForkCheckboxRow
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun DeleteBranchDialog(
    viewModel: DeleteConfirmDialogViewModel,
    branch: Branch,
    onDismiss: () -> Unit,
) {
    var force by remember { mutableStateOf(false) }
    var alsoDeleteRemote by remember { mutableStateOf(false) }
    val upstream by produceState<String?>(initialValue = null, branch) {
        val tracking = viewModel.upstreamOf(branch)
        value = tracking?.let { "${it.remote}/${it.branch}" }
    }

    ForkDialog(
        title = "Delete Branch",
        subtitle = branch.simpleName,
        primaryText = "Delete",
        onPrimary = {
            viewModel.deleteBranch(branch, force, alsoDeleteRemote && upstream != null)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        Text("Delete this branch?", fontSize = 12.sp)
        if (upstream != null) {
            ForkCheckboxRow("Also delete remote branch $upstream", alsoDeleteRemote, { alsoDeleteRemote = it })
        }
        ForkCheckboxRow("Force delete", force, { force = it })
    }
}

@Composable
fun DeleteTagDialog(
    viewModel: DeleteConfirmDialogViewModel,
    tag: Tag,
    onDismiss: () -> Unit,
) {
    ForkDialog(
        title = "Delete Tag",
        subtitle = tag.simpleName,
        primaryText = "Delete",
        onPrimary = {
            viewModel.deleteTag(tag)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        Text("Delete this tag?", fontSize = 12.sp)
    }
}

@Composable
fun DeleteStashDialog(
    viewModel: DeleteConfirmDialogViewModel,
    stash: Commit,
    onDismiss: () -> Unit,
) {
    ForkDialog(
        title = "Delete Stash",
        subtitle = stash.shortMessage,
        primaryText = "Delete",
        onPrimary = {
            viewModel.deleteStash(stash)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        Text("Delete this stash?", fontSize = 12.sp)
    }
}

@Composable
fun DeleteRemoteDialog(
    viewModel: DeleteConfirmDialogViewModel,
    remoteInfo: RemoteInfo,
    onDismiss: () -> Unit,
) {
    ForkDialog(
        title = "Delete Remote",
        subtitle = remoteInfo.remote.name,
        primaryText = "Delete",
        onPrimary = {
            viewModel.deleteRemote(remoteInfo)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        Text("Delete this remote?", fontSize = 12.sp)
    }
}
