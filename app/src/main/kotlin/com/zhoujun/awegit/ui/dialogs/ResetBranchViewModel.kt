package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import com.zhoujun.awegit.domain.usecases.ResetBranchUseCase
import com.zhoujun.awegit.domain.usecases.ResetType
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

class ResetBranchViewModel @AssistedInject constructor(
    private val resetBranchUseCase: ResetBranchUseCase,
    repositoryDataRepository: RepositoryDataRepository,
    @Assisted val targetCommit: Commit,
) : TabViewModel() {
    @AssistedFactory
    interface Factory {
        fun create(commit: Commit): ResetBranchViewModel
    }

    val branchName: Flow<String> = repositoryDataRepository.currentBranch.map {
        it.dataOrNull()?.simpleName ?: "HEAD"
    }

    fun reset(resetType: ResetType) {
        resetBranchUseCase(targetCommit, resetType)
    }
}
