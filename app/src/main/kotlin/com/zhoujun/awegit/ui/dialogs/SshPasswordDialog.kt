package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.Icon
import androidx.compose.material.IconButton
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.visibility
import com.zhoujun.awegit.app.generated.resources.visibility_off
import com.zhoujun.awegit.domain.credentials.CredentialsRequest
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog
import org.jetbrains.compose.resources.painterResource

@Composable
fun SshPasswordDialog(
    onReject: () -> Unit,
    onAccept: (password: String) -> Unit,
    credentialsRequest: CredentialsRequest.SshCredentialsRequest,
) {
    PasswordPromptDialog(
        title = "SSH Credentials",
        subtitle = "Your SSH key is protected with a password",
        initialPassword = credentialsRequest.password,
        isRetry = credentialsRequest.isRetry,
        retryMessage = "Invalid password, please try again",
        secondaryText = "Cancel",
        onDismiss = onReject,
        onAccept = onAccept,
    )
}

@Composable
internal fun PasswordPromptDialog(
    title: String,
    subtitle: String,
    initialPassword: String,
    isRetry: Boolean,
    retryMessage: String,
    secondaryText: String,
    onDismiss: () -> Unit,
    onAccept: (password: String) -> Unit,
) {
    var showRetryMessage by remember(isRetry) { mutableStateOf(isRetry) }
    var showPassword by remember { mutableStateOf(false) }
    var passwordField by remember {
        mutableStateOf(TextFieldValue(initialPassword, selection = TextRange(initialPassword.length)))
    }
    val labelWidth = 88.dp

    ForkDialog(
        title = title,
        subtitle = subtitle,
        primaryText = "Continue",
        secondaryText = secondaryText,
        onPrimary = { onAccept(passwordField.text) },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Password:", labelWidth = labelWidth) {
            ForkTextField(
                value = passwordField,
                onValueChange = {
                    passwordField = it
                    showRetryMessage = false
                },
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
        if (showRetryMessage) {
            Box(
                modifier = Modifier
                    .padding(top = 8.dp)
                    .clip(RoundedCornerShape(4.dp))
                    .background(MaterialTheme.colors.error)
                    .fillMaxWidth()
                    .padding(vertical = 4.dp, horizontal = 8.dp),
            ) {
                Text(retryMessage, color = MaterialTheme.colors.onError, fontSize = 12.sp)
            }
        }
    }
}
