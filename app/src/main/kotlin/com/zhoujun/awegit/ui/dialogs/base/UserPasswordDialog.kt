package com.zhoujun.awegit.ui.dialogs.base

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.Icon
import androidx.compose.material.IconButton
import androidx.compose.material.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.visibility
import com.zhoujun.awegit.app.generated.resources.visibility_off
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import org.jetbrains.compose.resources.painterResource

@Composable
fun UserPasswordDialog(
    title: String,
    subtitle: String,
    @Suppress("UNUSED_PARAMETER") icon: Painter?,
    onDismiss: () -> Unit,
    onAccept: (user: String, password: String) -> Unit,
) {
    var userField by remember { mutableStateOf(TextFieldValue("")) }
    var passwordField by remember { mutableStateOf(TextFieldValue("")) }
    var showPassword by remember { mutableStateOf(false) }
    val labelWidth = 88.dp

    ForkDialog(
        title = title,
        subtitle = subtitle,
        primaryText = "Continue",
        onPrimary = { onAccept(userField.text, passwordField.text) },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Username:", labelWidth = labelWidth) {
            ForkTextField(
                value = userField,
                onValueChange = { userField = it },
                modifier = Modifier.weight(1f),
            )
        }
        ForkFormRow("Password:", labelWidth = labelWidth) {
            ForkTextField(
                value = passwordField,
                onValueChange = { passwordField = it },
                modifier = Modifier.weight(1f),
                visualTransformation = if (showPassword) VisualTransformation.None else PasswordVisualTransformation(),
            )
            Spacer(Modifier.width(4.dp))
            IconButton(
                onClick = { showPassword = !showPassword },
                modifier = Modifier.size(24.dp).align(Alignment.CenterVertically),
            ) {
                Icon(
                    painterResource(if (showPassword) Res.drawable.visibility_off else Res.drawable.visibility),
                    contentDescription = null,
                    tint = MaterialTheme.colors.onBackground,
                    modifier = Modifier.size(16.dp),
                )
            }
        }
    }
}
