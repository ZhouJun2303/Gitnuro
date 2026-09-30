package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.domain.models.Remote
import com.zhoujun.awegit.theme.onBackgroundSecondary
import com.zhoujun.awegit.ui.components.fork.ForkButton
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

private enum class RepositorySettingsTab { General, Remotes, Ignore, GitFlow }

@Composable
fun RepositorySettingsDialog(
    viewModel: RepositorySettingsViewModel,
    onDismiss: () -> Unit,
) {
    var tab by remember { mutableStateOf(RepositorySettingsTab.General) }
    val author by viewModel.author.collectAsState()
    val remotes by viewModel.remotes.collectAsState()
    val gitFlow by viewModel.gitFlow.collectAsState()
    val ignoreText by viewModel.ignoreText.collectAsState()
    var name by remember(author) { mutableStateOf(author?.repositoryIdentity?.name.orEmpty()) }
    var email by remember(author) { mutableStateOf(author?.repositoryIdentity?.email.orEmpty()) }
    var remoteName by remember { mutableStateOf("") }
    var remoteUrl by remember { mutableStateOf("") }

    ForkDialog(
        title = "Repository Settings",
        subtitle = null,
        primaryText = "Save",
        onPrimary = {
            when (tab) {
                RepositorySettingsTab.General -> viewModel.saveAuthor(name, email)
                RepositorySettingsTab.Ignore -> viewModel.saveIgnore()
                RepositorySettingsTab.GitFlow -> viewModel.saveGitFlow()
                RepositorySettingsTab.Remotes -> Unit
            }
            onDismiss()
        },
        onDismiss = onDismiss,
        width = 560.dp,
    ) {
        Row(Modifier.padding(bottom = 8.dp)) {
            for (page in RepositorySettingsTab.entries) {
                Text(
                    page.name,
                    fontSize = 12.sp,
                    fontWeight = if (page == tab) FontWeight.SemiBold else FontWeight.Normal,
                    color = if (page == tab) MaterialTheme.colors.primary else MaterialTheme.colors.onBackgroundSecondary,
                    modifier = Modifier
                        .clickable { tab = page }
                        .padding(end = 12.dp, bottom = 4.dp),
                )
            }
        }
        when (tab) {
            RepositorySettingsTab.General -> {
                ForkFormRow("user.name", labelWidth = 80.dp) {
                    ForkTextField(name, { name = it }, modifier = Modifier.fillMaxWidth())
                }
                ForkFormRow("user.email", labelWidth = 80.dp) {
                    ForkTextField(email, { email = it }, modifier = Modifier.fillMaxWidth())
                }
            }
            RepositorySettingsTab.Remotes -> {
                Column(Modifier.heightIn(max = 180.dp).verticalScroll(rememberScrollState())) {
                    for (remoteInfo in remotes) {
                        EditRemoteRow(remoteInfo.remote) { viewModel.updateRemote(it) }
                        ForkButton("Delete ${remoteInfo.remote.name}", { viewModel.deleteRemote(remoteInfo) })
                    }
                }
                ForkFormRow("Name", labelWidth = 56.dp) {
                    ForkTextField(remoteName, { remoteName = it }, modifier = Modifier.fillMaxWidth())
                }
                ForkFormRow("URL", labelWidth = 56.dp) {
                    ForkTextField(remoteUrl, { remoteUrl = it }, modifier = Modifier.fillMaxWidth())
                }
                ForkButton(
                    "Add remote",
                    {
                        if (remoteName.isNotBlank() && remoteUrl.isNotBlank()) {
                            viewModel.addRemote(remoteName, remoteUrl)
                            remoteName = ""
                            remoteUrl = ""
                        }
                    },
                    primary = true,
                )
            }
            RepositorySettingsTab.Ignore -> {
                ForkTextField(
                    value = ignoreText,
                    onValueChange = viewModel::onIgnoreChange,
                    modifier = Modifier.fillMaxWidth().heightIn(min = 160.dp),
                    singleLine = false,
                    minHeight = 160.dp,
                )
            }
            RepositorySettingsTab.GitFlow -> {
                GitFlowConfigFields(gitFlow, labelWidth = 100.dp, onChange = viewModel::onGitFlowChange)
            }
        }
    }
}

@Composable
fun EditRemoteRow(remote: Remote, onSave: (Remote) -> Unit) {
    var url by remember(remote) { mutableStateOf(remote.fetchUri) }
    ForkFormRow(remote.name, labelWidth = 72.dp) {
        ForkTextField(url, { url = it }, modifier = Modifier.weight(1f))
        ForkButton("Save", { onSave(remote.copy(fetchUri = url, pushUri = url)) })
    }
}
