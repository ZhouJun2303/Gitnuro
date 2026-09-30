package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import com.zhoujun.awegit.domain.usecases.RebaseBranchUseCase
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

class RebaseDialogViewModel @AssistedInject constructor(
    private val rebaseBranchUseCase: RebaseBranchUseCase,
    repositoryDataRepository: RepositoryDataRepository,
    @Assisted val branch: Branch,
) : TabViewModel() {
    @AssistedFactory
    interface Factory {
        fun create(branch: Branch): RebaseDialogViewModel
    }

    val currentBranchName: Flow<String> = repositoryDataRepository.currentBranch.map {
        it.dataOrNull()?.simpleName ?: "HEAD"
    }

    fun rebase() {
        rebaseBranchUseCase(branch)
    }
}
