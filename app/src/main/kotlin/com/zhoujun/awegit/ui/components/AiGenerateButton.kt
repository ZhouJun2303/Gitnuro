package com.zhoujun.awegit.ui.components

import androidx.compose.foundation.layout.size
import androidx.compose.material.CircularProgressIndicator
import androidx.compose.material.Icon
import androidx.compose.material.IconButton
import androidx.compose.material.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.auto_awesome
import com.zhoujun.awegit.ui.components.tooltip.InstantTooltip
import org.jetbrains.compose.resources.painterResource

@Composable
fun AiGenerateButton(
    modifier: Modifier = Modifier,
    isGenerating: Boolean,
    enabled: Boolean,
    onGenerate: () -> Unit,
    onCancel: () -> Unit,
) {
    val tooltip = when {
        isGenerating -> "Stop generating"
        !enabled -> "Stage some changes first"
        else -> "Generate commit message with AI"
    }
    InstantTooltip(text = tooltip, modifier = modifier) {
        IconButton(
            onClick = { if (isGenerating) onCancel() else onGenerate() },
            enabled = enabled || isGenerating,
            modifier = Modifier.size(28.dp),
        ) {
            if (isGenerating) {
                CircularProgressIndicator(
                    modifier = Modifier.size(16.dp),
                    strokeWidth = 2.dp,
                    color = MaterialTheme.colors.primary,
                )
            } else {
                Icon(
                    painterResource(Res.drawable.auto_awesome),
                    contentDescription = tooltip,
                    modifier = Modifier.size(18.dp),
                    tint = MaterialTheme.colors.onBackground,
                )
            }
        }
    }
}
