package com.zhoujun.awegit.ui.dialogs

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.zhoujun.awegit.ui.components.fork.ForkCheckboxRow
import com.zhoujun.awegit.ui.components.fork.ForkDropdown
import com.zhoujun.awegit.ui.components.fork.ForkFormRow
import com.zhoujun.awegit.ui.dialogs.base.ForkDialog

@Composable
fun FetchDialog(viewModel: FetchDialogViewModel, onDismiss: () -> Unit) {
    val remotes by viewModel.remotes.collectAsState(emptyList())
    var selectedName by remember { mutableStateOf("All remotes") }
    var fetchAllTags by remember { mutableStateOf(false) }
    var prune by remember { mutableStateOf(true) }
    val options = listOf("All remotes") + remotes.map { it.name }
    ForkDialog(
        title = "Fetch",
        subtitle = null,
        primaryText = "Fetch",
        onPrimary = {
            val remote = remotes.firstOrNull { it.name == selectedName }
            viewModel.fetch(remote, fetchAllTags, prune)
            onDismiss()
        },
        onDismiss = onDismiss,
    ) {
        ForkFormRow("Remote") {
            ForkDropdown(
                items = options,
                selected = selectedName,
                itemLabel = { it },
                onSelected = { selectedName = it },
            )
        }
        ForkCheckboxRow("Fetch all tags", fetchAllTags, { fetchAllTags = it })
        ForkCheckboxRow("Prune remote branches", prune, { prune = it })
    }
}
