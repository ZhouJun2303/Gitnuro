package com.zhoujun.awegit.ui.dialogs

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import com.zhoujun.awegit.ui.components.fork.ForkCheckboxRow
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun CheckoutRemoteBranchDialog(
    viewModel: CheckoutRemoteBranchDialogViewModel,
    onDismiss: () -> Unit,
) {
    val initialName = viewModel.branch.simpleName
    var localName by remember {
        mutableStateOf(TextFieldValue(initialName, TextRange(0, initialName.length)))
    }
    var track by remember { mutableStateOf(true) }
    val labelWidth = 88.dp

    ForkDialog(
        title = "Checkout Branch",
        subtitle = viewModel.branch.simpleNameWithRemote,
        primaryText = "Checkout",
        primaryEnabled = localName.text.isNotBlank(),
        onPrimary = {
            viewModel.checkout(localName.text, track)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Local name:", labelWidth = labelWidth) {
            ForkTextField(
                value = localName,
                onValueChange = { localName = it },
                modifier = Modifier.weight(1f),
            )
        }
        ForkCheckboxRow("Track remote branch", track, { track = it }, labelWidth = labelWidth)
    }
}
