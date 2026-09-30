package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.domain.models.Workspace

sealed interface WorkspaceDialog {
    data object Create : WorkspaceDialog
    data class Rename(val workspace: Workspace) : WorkspaceDialog
    data class Delete(val workspace: Workspace) : WorkspaceDialog
    data object Manager : WorkspaceDialog
}
