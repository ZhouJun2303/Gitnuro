package com.zhoujun.awegit.ui.dialogs

import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.domain.usecases.ResetType
import com.zhoujun.awegit.theme.ForkDimens
import com.zhoujun.awegit.theme.onBackgroundSecondary
import com.zhoujun.awegit.ui.components.fork.ForkDropdown
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.unit.dp

@Composable
fun ResetBranchDialog(
    viewModel: ResetBranchViewModel,
    onDismiss: () -> Unit,
) {
    val branchName by viewModel.branchName.collectAsState("HEAD")
    var resetType by remember { mutableStateOf(ResetType.MIXED) }

    ForkDialog(
        title = "Reset Branch",
        subtitle = "Reset $branchName to ${viewModel.targetCommit.shortHash}",
        primaryText = "Reset",
        onPrimary = {
            viewModel.reset(resetType)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Mode:") {
            ForkDropdown(
                items = ResetType.entries,
                selected = resetType,
                itemLabel = { it.label() },
                onSelected = { resetType = it },
                modifier = Modifier.weight(1f),
            )
        }
        Text(
            resetType.description(),
            fontSize = 12.sp,
            color = MaterialTheme.colors.onBackgroundSecondary,
            modifier = Modifier.padding(start = ForkDimens.LabelWidth + ForkDimens.LabelGap, top = 2.dp),
        )
    }
}

private fun ResetType.label(): String = when (this) {
    ResetType.SOFT -> "Soft"
    ResetType.MIXED -> "Mixed"
    ResetType.HARD -> "Hard"
}

private fun ResetType.description(): String = when (this) {
    ResetType.SOFT -> "Keep the changes in the index (staged)"
    ResetType.MIXED -> "Keep the changes (unstaged)"
    ResetType.HARD -> "Discard all the changes"
}
