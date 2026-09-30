package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.theme.onBackgroundSecondary
import com.zhoujun.awegit.ui.components.AiGenerateButton
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun RewordCommitDialog(viewModel: RewordCommitDialogViewModel, onDismiss: () -> Unit) {
    val summary by viewModel.summary.collectAsState()
    val description by viewModel.description.collectAsState()
    val isGenerating by viewModel.isGenerating.collectAsState()
    val isAiEnabled by viewModel.isAiEnabled.collectAsState(false)
    val length = summary.text.length
    ForkDialog(
        title = "Edit Commit Message",
        subtitle = "Commit ${viewModel.commit.hash.take(7)} and the commits after it will get new hashes.",
        primaryText = "Save",
        onPrimary = {
            viewModel.save()
            onDismiss()
        },
        onDismiss = onDismiss,
        primaryEnabled = summary.text.isNotBlank(),
        width = 560.dp,
        acceptOnEnter = false,
    ) {
        ForkFormRow("Summary:", labelWidth = 76.dp) {
            ForkTextField(
                value = summary,
                onValueChange = { viewModel.summary.value = it },
                modifier = Modifier.weight(1f),
                singleLine = true,
                enabled = !isGenerating,
            )
            Spacer(Modifier.width(8.dp))
            Text(
                "$length/72",
                fontSize = 12.sp,
                color = if (length > 72) Color.Red else MaterialTheme.colors.onBackgroundSecondary,
            )
        }
        ForkFormRow("Description:", labelWidth = 76.dp) {
            ForkTextField(
                value = description,
                onValueChange = { viewModel.description.value = it },
                modifier = Modifier.weight(1f),
                singleLine = false,
                minHeight = 140.dp,
                enabled = !isGenerating,
            )
            if (isAiEnabled) {
                AiGenerateButton(
                    isGenerating = isGenerating,
                    enabled = true,
                    onGenerate = viewModel::generate,
                    onCancel = viewModel::cancelGenerate,
                )
            }
        }
    }
}
