package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.theme.backgroundSelected
import com.zhoujun.awegit.theme.onBackgroundSecondary
import com.zhoujun.awegit.ui.components.fork.ForkTextField
import com.zhoujun.awegit.ui.dialogs.base.MaterialDialog

@Composable
fun QuickLaunchDialog(
    viewModel: QuickLaunchViewModel,
    onDismiss: () -> Unit,
    onCommand: (QuickLaunchAction.Command) -> Unit,
) {
    val query by viewModel.query.collectAsState()
    val items by viewModel.results.collectAsState()
    var selected by remember(items) { mutableIntStateOf(0) }
    val focusRequester = remember { FocusRequester() }
    val listState = rememberLazyListState()

    fun runSelected() {
        val item = items.getOrNull(selected) ?: return
        when (val action = item.action) {
            is QuickLaunchAction.OpenRepository -> {
                viewModel.openRepository(action.path, action.workspaceId)
                onDismiss()
            }
            is QuickLaunchAction.Checkout -> {
                viewModel.checkout(action.branch)
                onDismiss()
            }
            is QuickLaunchAction.Command -> {
                onDismiss()
                onCommand(action)
            }
        }
    }

    MaterialDialog(paddingHorizontal = 0.dp, paddingVertical = 0.dp, onCloseRequested = onDismiss) {
        Column(
            Modifier
                .width(600.dp)
                .onPreviewKeyEvent { event ->
                    if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                    when (event.key) {
                        Key.DirectionDown -> {
                            if (items.isNotEmpty()) selected = (selected + 1).coerceAtMost(items.lastIndex)
                            true
                        }
                        Key.DirectionUp -> {
                            selected = (selected - 1).coerceAtLeast(0)
                            true
                        }
                        Key.Enter, Key.NumPadEnter -> {
                            runSelected()
                            true
                        }
                        else -> false
                    }
                }
                .padding(16.dp)
        ) {
            ForkTextField(
                value = query,
                onValueChange = viewModel::onQueryChange,
                modifier = Modifier.fillMaxWidth().focusRequester(focusRequester),
                placeholder = "Quick Launch",
            )
            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxWidth().heightIn(max = 360.dp).padding(top = 8.dp),
            ) {
                val grouped = items.groupBy { it.group }
                grouped.forEach { (group, rows) ->
                    item(key = "header-$group") {
                        Text(
                            group.name,
                            fontSize = 11.sp,
                            fontWeight = FontWeight.SemiBold,
                            color = MaterialTheme.colors.onBackgroundSecondary,
                            modifier = Modifier.padding(top = 8.dp, bottom = 2.dp),
                        )
                    }
                    itemsIndexed(rows, key = { _, row -> "${row.group}:${row.title}:${row.subtitle}" }) { _, row ->
                        val flatIndex = items.indexOf(row)
                        val active = flatIndex == selected
                        Column(
                            Modifier
                                .fillMaxWidth()
                                .background(if (active) MaterialTheme.colors.backgroundSelected else MaterialTheme.colors.background)
                                .clickable {
                                    selected = flatIndex
                                    runSelected()
                                }
                                .padding(horizontal = 8.dp, vertical = 4.dp)
                        ) {
                            Text(row.title, fontSize = 12.sp, maxLines = 1)
                            if (row.subtitle.isNotEmpty()) {
                                Text(row.subtitle, fontSize = 11.sp, color = MaterialTheme.colors.onBackgroundSecondary, maxLines = 1)
                            }
                        }
                    }
                }
            }
        }
    }

    LaunchedEffect(Unit) {
        focusRequester.requestFocus()
    }
    LaunchedEffect(selected) {
        if (items.isNotEmpty()) listState.animateScrollToItem(selected.coerceIn(0, items.lastIndex))
    }
}
