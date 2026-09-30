package com.zhoujun.awegit.ui.dialogs

import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import com.zhoujun.awegit.ui.components.fork.ForkCheckboxRow
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun CreateTagDialog(
    viewModel: CreateTagViewModel,
    onDismiss: () -> Unit,
) {
    var name by remember { mutableStateOf(TextFieldValue("")) }
    var message by remember { mutableStateOf(TextFieldValue("")) }
    var pushTag by remember { mutableStateOf(false) }
    val labelWidth = 80.dp

    ForkDialog(
        title = "Create Tag",
        subtitle = null,
        primaryText = "Create",
        primaryEnabled = name.text.isNotBlank(),
        onPrimary = {
            viewModel.createTag(name.text, message.text, pushTag)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Name:", labelWidth = labelWidth) {
            ForkTextField(name, { name = it }, Modifier.weight(1f))
        }
        ForkFormRow("Message:", labelWidth = labelWidth) {
            ForkTextField(
                value = message,
                onValueChange = { message = it },
                modifier = Modifier.weight(1f),
                singleLine = false,
                minHeight = 72.dp,
            )
        }
        ForkCheckboxRow("Push tag to origin", pushTag, { pushTag = it }, labelWidth = labelWidth)
    }
}
