package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.merge_automatic_stash_description
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import com.zhoujun.awegit.domain.usecases.MergeBranchUseCase
import com.zhoujun.awegit.domain.usecases.MergeMode
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.getString

class MergeDialogViewModel @AssistedInject constructor(
    private val mergeBranchUseCase: MergeBranchUseCase,
    repositoryDataRepository: RepositoryDataRepository,
    @Assisted val branch: Branch,
) : TabViewModel() {
    @AssistedFactory
    interface Factory {
        fun create(branch: Branch): MergeDialogViewModel
    }

    val currentBranchName: Flow<String> = repositoryDataRepository.currentBranch.map {
        it.dataOrNull()?.simpleName ?: "HEAD"
    }

    fun merge(mode: MergeMode) {
        viewModelScope.launch {
            val current = currentBranchName.first()
            val description = getString(
                Res.string.merge_automatic_stash_description,
                branch.simpleNameWithRemote,
                current,
            )
            mergeBranchUseCase(branch, description, mode)
        }
    }
}
