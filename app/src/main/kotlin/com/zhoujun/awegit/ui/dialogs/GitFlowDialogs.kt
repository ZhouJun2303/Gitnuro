package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.models.GitFlowBranchType
import com.zhoujun.awegit.domain.models.GitFlowConfig
import com.zhoujun.awegit.domain.usecases.GitFlowFinishUseCase
import com.zhoujun.awegit.domain.usecases.GitFlowInitUseCase
import com.zhoujun.awegit.domain.usecases.GitFlowStartUseCase
import com.zhoujun.awegit.theme.ForkDimens
import com.zhoujun.awegit.ui.components.fork.ForkCheckboxRow
import com.zhoujun.awegit.ui.components.fork.ForkDropdown
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog
import javax.inject.Inject

class GitFlowInitViewModel @Inject constructor(
    private val gitFlowInitUseCase: GitFlowInitUseCase,
) : TabViewModel() {
    fun init(config: GitFlowConfig) = gitFlowInitUseCase(config)
}

class GitFlowStartViewModel @Inject constructor(
    private val gitFlowStartUseCase: GitFlowStartUseCase,
) : TabViewModel() {
    fun start(type: GitFlowBranchType, name: String) = gitFlowStartUseCase(type, name)
}

class GitFlowFinishViewModel @Inject constructor(
    private val gitFlowFinishUseCase: GitFlowFinishUseCase,
) : TabViewModel() {
    fun finish(type: GitFlowBranchType, name: String, deleteBranch: Boolean, tagMessage: String) =
        gitFlowFinishUseCase(type, name, deleteBranch, tagMessage)
}

@Composable
fun GitFlowInitDialog(
    viewModel: GitFlowInitViewModel,
    onDismiss: () -> Unit,
) {
    var config by remember { mutableStateOf(GitFlowConfig()) }
    ForkDialog(
        title = "Initialize Git Flow",
        subtitle = "Branch names and prefixes",
        primaryText = "Init",
        onPrimary = {
            viewModel.init(config)
            onDismiss()
        },
        onDismiss = onDismiss,
        primaryEnabled = config.master.isNotBlank() && config.develop.isNotBlank(),
    ) {
        GitFlowConfigFields(config, labelWidth = 120.dp) { config = it }
    }
}

@Composable
fun GitFlowStartDialog(
    viewModel: GitFlowStartViewModel,
    onDismiss: () -> Unit,
) {
    var type by remember { mutableStateOf(GitFlowBranchType.Feature) }
    var name by remember { mutableStateOf("") }
    ForkDialog(
        title = "Start Git Flow",
        subtitle = null,
        primaryText = "Start",
        onPrimary = {
            viewModel.start(type, name.trim())
            onDismiss()
        },
        onDismiss = onDismiss,
        primaryEnabled = name.isNotBlank(),
    ) {
        GitFlowTypeField(type) { type = it }
        ForkFormRow("Name", labelWidth = 72.dp) {
            ForkTextField(name, { name = it }, modifier = Modifier.fillMaxWidth())
        }
    }
}

@Composable
fun GitFlowFinishDialog(
    viewModel: GitFlowFinishViewModel,
    onDismiss: () -> Unit,
) {
    var type by remember { mutableStateOf(GitFlowBranchType.Feature) }
    var name by remember { mutableStateOf("") }
    var deleteBranch by remember { mutableStateOf(true) }
    var tagMessage by remember { mutableStateOf("") }
    ForkDialog(
        title = "Finish Git Flow",
        subtitle = null,
        primaryText = "Finish",
        onPrimary = {
            viewModel.finish(type, name.trim(), deleteBranch, tagMessage)
            onDismiss()
        },
        onDismiss = onDismiss,
        primaryEnabled = name.isNotBlank(),
    ) {
        GitFlowTypeField(type) { type = it }
        ForkFormRow("Name", labelWidth = 88.dp) {
            ForkTextField(name, { name = it }, modifier = Modifier.fillMaxWidth())
        }
        ForkCheckboxRow("Delete branch", deleteBranch, { deleteBranch = it }, labelWidth = 88.dp)
        if (type == GitFlowBranchType.Release || type == GitFlowBranchType.Hotfix) {
            ForkFormRow("Tag message", labelWidth = 88.dp) {
                ForkTextField(tagMessage, { tagMessage = it }, modifier = Modifier.fillMaxWidth())
            }
        }
    }
}

@Composable
fun GitFlowConfigFields(
    config: GitFlowConfig,
    labelWidth: androidx.compose.ui.unit.Dp = ForkDimens.LabelWidth,
    onChange: (GitFlowConfig) -> Unit,
) {
    val rows = listOf(
        "Production" to config.master,
        "Development" to config.develop,
        "Feature" to config.featurePrefix,
        "Release" to config.releasePrefix,
        "Hotfix" to config.hotfixPrefix,
        "Version tag" to config.versionTagPrefix,
    )
    for ((label, value) in rows) {
        ForkFormRow(label, labelWidth = labelWidth) {
            ForkTextField(
                value = value,
                onValueChange = { text ->
                    onChange(
                        when (label) {
                            "Production" -> config.copy(master = text)
                            "Development" -> config.copy(develop = text)
                            "Feature" -> config.copy(featurePrefix = text)
                            "Release" -> config.copy(releasePrefix = text)
                            "Hotfix" -> config.copy(hotfixPrefix = text)
                            else -> config.copy(versionTagPrefix = text)
                        }
                    )
                },
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

@Composable
private fun GitFlowTypeField(
    type: GitFlowBranchType,
    onType: (GitFlowBranchType) -> Unit,
) {
    ForkFormRow("Type", labelWidth = 72.dp) {
        ForkDropdown(
            items = GitFlowBranchType.entries,
            selected = type,
            itemLabel = { it.name },
            onSelected = onType,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}
