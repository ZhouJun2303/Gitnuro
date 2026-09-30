package com.zhoujun.awegit.ui.dialogs

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun StashWithMessageDialog(
    viewModel: StashWithMessageViewModel,
    onDismiss: () -> Unit,
) {
    var message by remember { mutableStateOf(TextFieldValue("")) }

    ForkDialog(
        title = "Stash Changes",
        subtitle = null,
        primaryText = "Stash",
        primaryEnabled = message.text.isNotBlank(),
        onPrimary = {
            viewModel.stash(message.text)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Message:", labelWidth = 80.dp) {
            ForkTextField(
                value = message,
                onValueChange = { message = it },
                modifier = Modifier.weight(1f),
            )
        }
    }
}
