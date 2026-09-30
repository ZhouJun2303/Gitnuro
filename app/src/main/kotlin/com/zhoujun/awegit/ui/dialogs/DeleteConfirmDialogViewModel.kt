package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.errors.okOrNull
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.RemoteInfo
import com.zhoujun.awegit.domain.models.Tag
import com.zhoujun.awegit.domain.models.TrackingBranch
import com.zhoujun.awegit.domain.usecases.DeleteBranchUseCase
import com.zhoujun.awegit.domain.usecases.DeleteRemoteBranchUseCase
import com.zhoujun.awegit.domain.usecases.DeleteRemoteInfoUseCase
import com.zhoujun.awegit.domain.usecases.DeleteStashUseCase
import com.zhoujun.awegit.domain.usecases.DeleteTagUseCase
import com.zhoujun.awegit.domain.usecases.GetTrackingBranchUseCase
import javax.inject.Inject
import kotlinx.coroutines.launch

class DeleteConfirmDialogViewModel @Inject constructor(
    private val deleteBranchUseCase: DeleteBranchUseCase,
    private val deleteRemoteBranchUseCase: DeleteRemoteBranchUseCase,
    private val deleteTagUseCase: DeleteTagUseCase,
    private val deleteStashUseCase: DeleteStashUseCase,
    private val deleteRemoteInfoUseCase: DeleteRemoteInfoUseCase,
    private val getTrackingBranchUseCase: GetTrackingBranchUseCase,
) : TabViewModel() {
    suspend fun upstreamOf(branch: Branch): TrackingBranch? = getTrackingBranchUseCase(branch).okOrNull()

    fun deleteBranch(branch: Branch, force: Boolean, alsoDeleteRemote: Boolean) {
        deleteBranchUseCase(branch, force)
        if (alsoDeleteRemote) {
            viewModelScope.launch {
                val tracking = upstreamOf(branch) ?: return@launch
                deleteRemoteBranchUseCase(
                    Branch(
                        hash = branch.hash,
                        name = "refs/remotes/${tracking.remote}/${tracking.branch}",
                        isLocal = false,
                    ),
                )
            }
        }
    }

    fun deleteTag(tag: Tag) = deleteTagUseCase(tag)

    fun deleteStash(stash: Commit) = deleteStashUseCase(stash)

    fun deleteRemote(remoteInfo: RemoteInfo) = deleteRemoteInfoUseCase(remoteInfo)
}
