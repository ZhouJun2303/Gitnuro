package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import com.zhoujun.awegit.domain.usecases.CreateBranchUseCase
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map

class CreateBranchViewModel @AssistedInject constructor(
    private val createBranchUseCase: CreateBranchUseCase,
    repositoryDataRepository: RepositoryDataRepository,
    @Assisted val commit: Commit?,
) : TabViewModel() {
    @AssistedFactory
    interface Factory {
        fun create(commit: Commit?): CreateBranchViewModel
    }

    val atLabel: Flow<String> = if (commit != null) {
        flowOf(commit.shortHash)
    } else {
        repositoryDataRepository.currentBranch.map { it.dataOrNull()?.simpleName ?: "HEAD" }
    }

    fun createBranch(branchName: String, checkout: Boolean) {
        createBranchUseCase(branchName, commit, checkout)
    }
}
