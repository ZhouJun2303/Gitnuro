package com.zhoujun.awegit.ui.dialogs

import androidx.compose.runtime.Composable
import com.zhoujun.awegit.domain.credentials.CredentialsRequest

@Composable
fun GpgPasswordDialog(
    gpgCredentialsRequest: CredentialsRequest.GpgCredentialsRequest,
    onReject: () -> Unit,
    onAccept: (password: String) -> Unit,
) {
    PasswordPromptDialog(
        title = "GPG Credentials",
        subtitle = "Your GPG key is protected with a password",
        initialPassword = gpgCredentialsRequest.password,
        isRetry = gpgCredentialsRequest.isRetry,
        retryMessage = "Invalid password, please try again",
        secondaryText = "Do not sign",
        onDismiss = onReject,
        onAccept = onAccept,
    )
}
