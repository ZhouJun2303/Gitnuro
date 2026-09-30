package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.CircularProgressIndicator
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.clone_error_invalid_directory
import com.zhoujun.awegit.app.generated.resources.clone_error_invalid_folder_name
import com.zhoujun.awegit.app.generated.resources.clone_error_invalid_url
import com.zhoujun.awegit.domain.models.CloneState
import com.zhoujun.awegit.ui.components.fork.ForkButton
import com.zhoujun.awegit.ui.components.fork.ForkCheckboxRow
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog
import com.zhoujun.awegit.ui.getErrorText
import com.zhoujun.awegit.viewmodels.CloneUiError
import com.zhoujun.awegit.viewmodels.CloneViewModel
import org.jetbrains.compose.resources.stringResource
import java.io.File

private val CloneLabelWidth = 120.dp

@Composable
fun CloneDialog(
    cloneViewModel: CloneViewModel,
    onClose: () -> Unit,
    onOpenRepository: (File) -> Unit,
) {
    val cloneStatusValue by cloneViewModel.cloneState.collectAsState()
    var url by remember(cloneViewModel) { mutableStateOf(cloneViewModel.repositoryUrl.value) }
    var directory by remember(cloneViewModel) { mutableStateOf(cloneViewModel.directoryPath.value) }
    var folder by remember(cloneViewModel) { mutableStateOf(cloneViewModel.folder.value) }
    var cloneSubmodules by remember { mutableStateOf(true) }
    var nameEdited by remember(cloneViewModel) { mutableStateOf(false) }

    when (val status = cloneStatusValue) {
        is CloneState.Cloning -> ForkDialog(
            title = "Clone Repository",
            subtitle = status.taskName,
            primaryText = "Clone",
            primaryEnabled = false,
            onPrimary = {},
            onDismiss = { cloneViewModel.cancelClone() },
            width = 560.dp,
        ) {
            CloneProgress(status)
        }

        is CloneState.Cancelling -> ForkDialog(
            title = "Clone Repository",
            subtitle = "Cancelling clone operation...",
            primaryText = "Clone",
            primaryEnabled = false,
            onPrimary = {},
            onDismiss = {},
            width = 560.dp,
        ) {
            CircularProgressIndicator(color = MaterialTheme.colors.primaryVariant)
        }

        is CloneState.Completed -> {
            onOpenRepository(status.repoDir)
            onClose()
        }

        is CloneState.Fail, CloneState.None -> CloneForm(
            cloneViewModel = cloneViewModel,
            onClose = onClose,
            url = url,
            directory = directory,
            folder = folder,
            cloneSubmodules = cloneSubmodules,
            nameEdited = nameEdited,
            onUrlChange = { url = it },
            onDirectoryChange = { directory = it },
            onFolderChange = { folder = it },
            onCloneSubmodulesChange = { cloneSubmodules = it },
            onNameEdited = { nameEdited = true },
        )
    }
}

@Composable
private fun CloneForm(
    cloneViewModel: CloneViewModel,
    onClose: () -> Unit,
    url: TextFieldValue,
    directory: TextFieldValue,
    folder: TextFieldValue,
    cloneSubmodules: Boolean,
    nameEdited: Boolean,
    onUrlChange: (TextFieldValue) -> Unit,
    onDirectoryChange: (TextFieldValue) -> Unit,
    onFolderChange: (TextFieldValue) -> Unit,
    onCloneSubmodulesChange: (Boolean) -> Unit,
    onNameEdited: () -> Unit,
) {
    val error by cloneViewModel.error.collectAsState()
    val saveDirAsDefault by cloneViewModel.saveDirAsDefault.collectAsState()

    ForkDialog(
        title = "Clone Repository",
        subtitle = null,
        primaryText = "Clone",
        primaryEnabled = url.text.isNotBlank() && directory.text.isNotBlank() && folder.text.isNotBlank(),
        onPrimary = {
            cloneViewModel.clone(directory.text, url.text, folder.text, cloneSubmodules)
        },
        onDismiss = onClose,
        width = 560.dp,
    ) {
        ForkFormRow("Repository URL:", labelWidth = CloneLabelWidth) {
            ForkTextField(
                value = url,
                onValueChange = { repositoryUrl ->
                    onUrlChange(repositoryUrl)
                    cloneViewModel.onRepositoryUrlChanged(repositoryUrl)
                    if (!nameEdited) {
                        val derived = TextFieldValue(cloneViewModel.repoName(repositoryUrl.text))
                        onFolderChange(derived)
                        cloneViewModel.onFolderNameChanged(derived)
                    }
                    cloneViewModel.resetStateIfError()
                },
                modifier = Modifier.weight(1f),
            )
        }
        ForkFormRow("Parent Folder:", labelWidth = CloneLabelWidth) {
            ForkTextField(
                value = directory,
                onValueChange = {
                    onDirectoryChange(it)
                    cloneViewModel.onDirectoryPathChanged(it)
                    cloneViewModel.resetStateIfError()
                },
                modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(8.dp))
            ForkButton(
                text = "Browse…",
                onClick = {
                    cloneViewModel.resetStateIfError()
                    val newDirectory = cloneViewModel.openDirectoryPicker()
                    if (newDirectory != null) {
                        val value = TextFieldValue(newDirectory, selection = TextRange(newDirectory.count()))
                        onDirectoryChange(value)
                        cloneViewModel.onDirectoryPathChanged(value)
                        cloneViewModel.resetStateIfError()
                    }
                },
            )
        }
        ForkCheckboxRow(
            text = "Save as default",
            checked = saveDirAsDefault,
            onCheckedChange = { cloneViewModel.onSaveAsDefaultChanged(it) },
            labelWidth = CloneLabelWidth,
        )
        ForkFormRow("Name:", labelWidth = CloneLabelWidth) {
            ForkTextField(
                value = folder,
                onValueChange = { folderName ->
                    onNameEdited()
                    onFolderChange(folderName)
                    cloneViewModel.onFolderNameChanged(folderName)
                    cloneViewModel.resetStateIfError()
                },
                modifier = Modifier.weight(1f),
            )
        }
        ForkCheckboxRow(
            text = "Clone submodules recursively",
            checked = cloneSubmodules,
            onCheckedChange = onCloneSubmodulesChange,
            labelWidth = CloneLabelWidth,
        )
        if (error != null) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp)
                    .clip(RoundedCornerShape(4.dp))
                    .background(MaterialTheme.colors.error),
            ) {
                Text(
                    error?.toUiString().orEmpty(),
                    modifier = Modifier.padding(vertical = 4.dp, horizontal = 8.dp),
                    color = MaterialTheme.colors.onError,
                    fontSize = 12.sp,
                )
            }
        }
    }
}

@Composable
private fun CloneUiError.toUiString(): String {
    return when (this) {
        is CloneUiError.CloneError -> this.error.getErrorText()
        CloneUiError.EmptyDirectory -> stringResource(Res.string.clone_error_invalid_directory)
        CloneUiError.EmptyFolderName -> stringResource(Res.string.clone_error_invalid_folder_name)
        CloneUiError.EmptyUrl -> stringResource(Res.string.clone_error_invalid_url)
    }
}

@Composable
private fun CloneProgress(cloneStateValue: CloneState.Cloning) {
    val progress = remember(cloneStateValue) {
        val total = cloneStateValue.total
        if (total == 0) -1f else cloneStateValue.progress / total.toFloat()
    }
    if (progress >= 0f) {
        CircularProgressIndicator(
            progress = progress,
            color = MaterialTheme.colors.primaryVariant,
        )
    } else {
        CircularProgressIndicator(color = MaterialTheme.colors.primaryVariant)
    }
}
