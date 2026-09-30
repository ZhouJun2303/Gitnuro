package com.zhoujun.awegit.ui.dialogs

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun AddEditRemoteDialog(
    viewModel: AddEditRemoteViewModel,
    onDismiss: () -> Unit,
) {
    val remote by viewModel.remote.collectAsState()
    val isNew = viewModel.isNewRemote
    var name by remember(viewModel) {
        mutableStateOf(TextFieldValue(remote.name, TextRange(remote.name.length)))
    }
    var url by remember(viewModel) {
        mutableStateOf(TextFieldValue(remote.fetchUri, TextRange(remote.fetchUri.length)))
    }
    var pushUrl by remember(viewModel) {
        mutableStateOf(TextFieldValue(remote.pushUri, TextRange(remote.pushUri.length)))
    }
    val labelWidth = 80.dp

    ForkDialog(
        title = if (isNew) "Add Remote" else "Edit Remote",
        subtitle = null,
        primaryText = if (isNew) "Add" else "Save",
        primaryEnabled = remote.name.isNotBlank() && remote.fetchUri.isNotBlank(),
        onPrimary = {
            viewModel.save()
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Name:", labelWidth = labelWidth) {
            ForkTextField(
                value = name,
                onValueChange = {
                    name = it
                    viewModel.updateRemoteName(it.text)
                },
                enabled = isNew,
                modifier = Modifier.weight(1f),
            )
        }
        ForkFormRow("URL:", labelWidth = labelWidth) {
            ForkTextField(
                value = url,
                onValueChange = {
                    url = it
                    if (pushUrl.text == remote.fetchUri) {
                        pushUrl = it
                        viewModel.updateAllUri(it.text)
                    } else {
                        viewModel.updateFetchUri(it.text)
                    }
                },
                modifier = Modifier.weight(1f),
            )
        }
        ForkFormRow("Push URL:", labelWidth = labelWidth) {
            ForkTextField(
                value = pushUrl,
                onValueChange = {
                    pushUrl = it
                    viewModel.updatePushUri(it.text)
                },
                modifier = Modifier.weight(1f),
            )
        }
    }
}
