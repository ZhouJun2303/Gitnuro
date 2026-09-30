package com.zhoujun.awegit.ui.context_menu

import com.zhoujun.awegit.app.generated.resources.*
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.apply_stash
import com.zhoujun.awegit.app.generated.resources.delete
import com.zhoujun.awegit.app.generated.resources.stashes_context_menu_apply_stash
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource

fun stashesContextMenuItems(
    onApply: () -> Unit,
    onPop: () -> Unit,
    onDelete: () -> Unit,
): List<ContextMenuElement> {
    return listOf(
        ContextMenuElement.ContextTextEntry(
            composableLabel = { stringResource(Res.string.stashes_context_menu_apply_stash) },
            icon = { painterResource(Res.drawable.apply_stash) },
            onClick = onApply
        ),
        ContextMenuElement.ContextTextEntry(
            composableLabel = { stringResource(Res.string.stashes_context_menu_pop_stash) },
            icon = { painterResource(Res.drawable.apply_stash) },
            onClick = onPop
        ),
        ContextMenuElement.ContextTextEntry(
            composableLabel = { stringResource(Res.string.stashes_context_menu_drop_stash) },
            icon = { painterResource(Res.drawable.delete) },
            onClick = onDelete
        ),
    )
}