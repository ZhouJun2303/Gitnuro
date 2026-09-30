package com.zhoujun.awegit.ui.dialogs.base

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.Icon
import androidx.compose.material.IconButton
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.close
import com.zhoujun.awegit.app.generated.resources.logo
import com.zhoujun.awegit.keybindings.KeybindingOption
import com.zhoujun.awegit.keybindings.matchesBinding
import com.zhoujun.awegit.theme.ForkDimens
import com.zhoujun.awegit.theme.onBackgroundSecondary
import com.zhoujun.awegit.ui.components.fork.ForkButton
import org.jetbrains.compose.resources.painterResource

@Composable
fun ForkDialog(
    title: String,
    subtitle: String?,
    primaryText: String,
    onPrimary: () -> Unit,
    onDismiss: () -> Unit,
    primaryEnabled: Boolean = true,
    secondaryText: String = "Cancel",
    width: Dp = ForkDimens.DialogWidth,
    acceptOnEnter: Boolean = true,
    content: @Composable ColumnScope.() -> Unit,
) {
    MaterialDialog(paddingHorizontal = 0.dp, paddingVertical = 0.dp, onCloseRequested = onDismiss) {
        Box(
            Modifier
                .width(width)
                .onPreviewKeyEvent { e ->
                    val accept = (acceptOnEnter && e.matchesBinding(KeybindingOption.SIMPLE_ACCEPT)) ||
                        e.matchesBinding(KeybindingOption.TEXT_ACCEPT)
                    if (accept && primaryEnabled) {
                        onPrimary()
                        true
                    } else {
                        false
                    }
                }
        ) {
            Row(Modifier.padding(start = 24.dp, top = 24.dp, end = 20.dp, bottom = 20.dp)) {
                Image(
                    painterResource(Res.drawable.logo),
                    contentDescription = null,
                    modifier = Modifier.size(ForkDimens.DialogIconSize),
                )
                Spacer(Modifier.width(20.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        title,
                        fontSize = 16.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colors.onBackground,
                    )
                    if (subtitle != null) {
                        Text(
                            subtitle,
                            fontSize = 12.sp,
                            color = MaterialTheme.colors.onBackgroundSecondary,
                            modifier = Modifier.padding(top = 2.dp),
                        )
                    }
                    Spacer(Modifier.height(14.dp))
                    content()
                    Spacer(Modifier.height(16.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                        ForkButton(primaryText, onPrimary, primary = true, enabled = primaryEnabled)
                        Spacer(Modifier.width(ForkDimens.ButtonGap))
                        ForkButton(secondaryText, onDismiss)
                    }
                }
            }
            IconButton(
                onClick = onDismiss,
                modifier = Modifier.align(Alignment.TopEnd).padding(6.dp).size(28.dp),
            ) {
                Icon(
                    painterResource(Res.drawable.close),
                    contentDescription = null,
                    modifier = Modifier.size(14.dp),
                    tint = MaterialTheme.colors.onBackground,
                )
            }
        }
    }
}
