package com.zhoujun.awegit.ui.context_menu

import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.push_context_menu_force_push
import com.zhoujun.awegit.app.generated.resources.push_context_menu_push_including_tags
import com.zhoujun.awegit.app.generated.resources.tag
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource

fun pushContextMenuItems(
    onPushWithTags: () -> Unit,
    onForcePush: () -> Unit,
): List<ContextMenuElement> {
    return mutableListOf(
        ContextMenuElement.ContextTextEntry(
            composableLabel = { stringResource(Res.string.push_context_menu_push_including_tags) },
            icon = { painterResource(Res.drawable.tag) },
            onClick = onPushWithTags,
        ),
        ContextMenuElement.ContextTextEntry(
            composableLabel = { stringResource(Res.string.push_context_menu_force_push) },
            onClick = onForcePush,
        ),
    )
}
