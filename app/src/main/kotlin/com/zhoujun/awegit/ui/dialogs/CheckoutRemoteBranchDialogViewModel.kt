package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.usecases.CheckoutBranchUseCase
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject

class CheckoutRemoteBranchDialogViewModel @AssistedInject constructor(
    private val checkoutBranchUseCase: CheckoutBranchUseCase,
    @Assisted val branch: Branch,
) : TabViewModel() {
    @AssistedFactory
    interface Factory {
        fun create(branch: Branch): CheckoutRemoteBranchDialogViewModel
    }

    fun checkout(localName: String, track: Boolean) {
        checkoutBranchUseCase(branch, localName, track)
    }
}
