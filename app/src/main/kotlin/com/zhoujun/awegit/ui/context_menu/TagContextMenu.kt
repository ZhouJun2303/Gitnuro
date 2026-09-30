package com.zhoujun.awegit.ui.context_menu

import com.zhoujun.awegit.app.generated.resources.*
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.delete
import com.zhoujun.awegit.app.generated.resources.start
import com.zhoujun.awegit.app.generated.resources.tag_context_menu_checkout_tag_commit
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource

fun tagContextMenuItems(
    onCheckoutTag: () -> Unit,
    onDeleteTag: () -> Unit,
): List<ContextMenuElement> {
    return mutableListOf(
        ContextMenuElement.ContextTextEntry(
            composableLabel = { stringResource(Res.string.tag_context_menu_checkout_tag_commit) },
            icon = { painterResource(Res.drawable.start) },
            onClick = onCheckoutTag
        ),
        ContextMenuElement.ContextTextEntry(
            composableLabel = { stringResource(Res.string.tag_context_menu_delete_tag) },
            icon = { painterResource(Res.drawable.delete) },
            onClick = onDeleteTag
        )
    )
}