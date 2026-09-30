package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.usecases.ResetBranchUseCase
import com.zhoujun.awegit.domain.usecases.ResetType
import com.zhoujun.awegit.domain.usecases.StashChangesUseCase
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import org.eclipse.jgit.revwalk.RevCommit
import javax.inject.Inject

class StashWithMessageViewModel @Inject constructor(
    private val stashChangesUseCase: StashChangesUseCase,
) : TabViewModel() {

    fun stash(message: String) {
        stashChangesUseCase(message)
    }
}