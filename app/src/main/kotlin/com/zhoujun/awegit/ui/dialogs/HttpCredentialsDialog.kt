package com.zhoujun.awegit.ui.dialogs

import androidx.compose.runtime.Composable
import com.zhoujun.awegit.ui.dialogs.base.UserPasswordDialog

@Composable
fun HttpCredentialsDialog(
    onDismiss: () -> Unit,
    onAccept: (user: String, password: String) -> Unit,
) {
    UserPasswordDialog(
        title = "HTTP Credentials",
        subtitle = "Your remote requires a username and a password",
        icon = null,
        onDismiss = onDismiss,
        onAccept = onAccept,
    )
}
