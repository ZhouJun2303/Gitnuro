package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.items
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.AppConstants
import com.zhoujun.awegit.AppConstants.openSourceProjects
import com.zhoujun.awegit.Project
import com.zhoujun.awegit.ui.components.ScrollableLazyColumn
import com.zhoujun.awegit.ui.components.TextLink
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun AppInfoDialog(
    onClose: () -> Unit,
    onOpenUrlInBrowser: (String) -> Unit,
) {
    ForkDialog(
        title = AppConstants.APP_NAME,
        subtitle = AppConstants.APP_DESCRIPTION,
        primaryText = "Close",
        onPrimary = onClose,
        onDismiss = onClose,
        width = 560.dp,
    ) {
        ScrollableLazyColumn(modifier = Modifier.height(420.dp).fillMaxWidth()) {
            item {
                Text(
                    "Copyright © 2026 ZhouJun. AweGit is a modified version of Gitnuro. It is free software under the GNU GPL v3 and comes with ABSOLUTELY NO WARRANTY.",
                    style = MaterialTheme.typography.body2,
                    fontSize = 12.sp,
                )
                Text(
                    "AweGit is built on top of the following open source projects:",
                    style = MaterialTheme.typography.body2,
                    fontSize = 12.sp,
                    modifier = Modifier.padding(top = 12.dp, bottom = 8.dp),
                )
            }
            items(openSourceProjects) {
                ProjectUsed(it, onOpenUrlInBrowser = onOpenUrlInBrowser)
            }
        }
    }
}

@Composable
fun ProjectUsed(
    project: Project,
    onOpenUrlInBrowser: (String) -> Unit,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.padding(horizontal = 4.dp),
    ) {
        TextLink(
            text = project.name,
            url = project.url,
            modifier = Modifier.padding(vertical = 8.dp),
            onClick = { onOpenUrlInBrowser(project.url) },
        )
        Spacer(Modifier.weight(1f))
        TextLink(
            text = project.license.name,
            url = project.license.url,
            modifier = Modifier.padding(vertical = 8.dp),
            colorsInverted = true,
            onClick = { onOpenUrlInBrowser(project.license.url) },
        )
    }
}
