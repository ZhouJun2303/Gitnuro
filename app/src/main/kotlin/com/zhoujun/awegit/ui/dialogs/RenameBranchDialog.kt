package com.zhoujun.awegit.ui.dialogs

import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.unit.dp
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog
import com.zhoujun.awegit.ui.getErrorText
import com.zhoujun.awegit.viewmodels.RenameBranchDialogViewModel
import com.zhoujun.awegit.viewmodels.RenameState

@Composable
fun RenameBranchDialog(
    viewModel: RenameBranchDialogViewModel,
    onDismiss: () -> Unit,
) {
    val branch = viewModel.branch
    var field by remember(branch) {
        val branchName = branch.simpleName
        mutableStateOf(
            TextFieldValue(
                text = branchName,
                selection = TextRange(0, branchName.count()),
            )
        )
    }
    val state by viewModel.renameState.collectAsState()

    LaunchedEffect(state) {
        if (state is RenameState.Success) {
            onDismiss()
        }
    }

    val canEdit = state is RenameState.Waiting || state is RenameState.Failed
    ForkDialog(
        title = "Rename Branch",
        subtitle = null,
        primaryText = "Rename",
        primaryEnabled = field.text.isNotBlank() && canEdit,
        onPrimary = { viewModel.renameBranch(branch, field.text) },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("New name:", labelWidth = 80.dp) {
            ForkTextField(
                value = field,
                onValueChange = { field = it },
                enabled = canEdit,
                modifier = Modifier.weight(1f),
            )
        }
        if (state is RenameState.Failed) {
            Text(
                (state as RenameState.Failed).error.getErrorText(),
                color = MaterialTheme.colors.error,
                fontSize = 12.sp,
            )
        }
    }
}
