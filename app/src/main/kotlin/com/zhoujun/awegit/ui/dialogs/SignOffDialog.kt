package com.zhoujun.awegit.ui.dialogs

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
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
fun SignOffDialog(
    viewModel: SignOffDialogViewModel,
    onDismiss: () -> Unit,
) {
    val state = viewModel.state.collectAsState().value
    LaunchedEffect(viewModel) {
        viewModel.loadSignOffFormat()
    }

    var signOffField by remember(viewModel, state) {
        val signOff = if (state is SignOffState.Loaded) state.signOffConfig.format else ""
        mutableStateOf(TextFieldValue(signOff, TextRange(signOff.count())))
    }
    var enabledSignOff by remember(viewModel, state) {
        val signOff = if (state is SignOffState.Loaded) state.signOffConfig.isEnabled else true
        mutableStateOf(signOff)
    }
    val loaded = state is SignOffState.Loaded
    val labelWidth = 72.dp

    ForkDialog(
        title = "Sign-off",
        subtitle = "Enable or disable the sign-off or adjust its format",
        primaryText = "Save",
        primaryEnabled = loaded,
        onPrimary = {
            viewModel.saveSignOffFormat(enabledSignOff, signOffField.text)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Format:", labelWidth = labelWidth) {
            ForkTextField(
                value = signOffField,
                onValueChange = { signOffField = it },
                enabled = loaded,
                modifier = Modifier.weight(1f),
            )
        }
        ForkCheckboxRow(
            text = "Enable sign-off for this repository",
            checked = enabledSignOff,
            onCheckedChange = { if (loaded) enabledSignOff = it },
            labelWidth = labelWidth,
        )
    }
}
