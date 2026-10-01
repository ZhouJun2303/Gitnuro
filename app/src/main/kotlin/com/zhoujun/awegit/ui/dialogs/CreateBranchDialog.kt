package com.zhoujun.awegit.ui.dialogs

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.input.TextFieldValue
import com.zhoujun.awegit.ui.components.fork.ForkCheckboxRow
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun CreateBranchDialog(
    viewModel: CreateBranchViewModel,
    onDismiss: () -> Unit,
) {
    val atLabel by viewModel.atLabel.collectAsState("HEAD")
    var name by remember { mutableStateOf(TextFieldValue("")) }
    var checkout by remember { mutableStateOf(true) }

    ForkDialog(
        title = "Create Branch",
        subtitle = "Create a new branch at $atLabel",
        primaryText = "Create",
        primaryEnabled = name.text.isNotBlank(),
        onPrimary = {
            viewModel.createBranch(name.text, checkout)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Name:") {
            ForkTextField(
                value = name,
                onValueChange = { name = it },
                modifier = Modifier.weight(1f),
            )
        }
        ForkCheckboxRow("Check out after create", checkout, { checkout = it })
    }
}
