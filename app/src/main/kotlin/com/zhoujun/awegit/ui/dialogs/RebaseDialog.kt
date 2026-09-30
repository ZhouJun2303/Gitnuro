package com.zhoujun.awegit.ui.dialogs

import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun RebaseDialog(
    viewModel: RebaseDialogViewModel,
    onDismiss: () -> Unit,
) {
    val currentBranch by viewModel.currentBranchName.collectAsState("HEAD")
    val onto = viewModel.branch.simpleNameWithRemote

    ForkDialog(
        title = "Rebase",
        subtitle = "Rebase $currentBranch onto $onto",
        primaryText = "Rebase",
        onPrimary = {
            viewModel.rebase()
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        Text(
            "Replay commits from $currentBranch onto $onto.",
            fontSize = 12.sp,
        )
    }
}
