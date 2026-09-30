package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import com.zhoujun.awegit.domain.usecases.PushBranchUseCase
import javax.inject.Inject
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

class PushDialogViewModel @Inject constructor(
    repositoryDataRepository: RepositoryDataRepository,
    private val pushBranchUseCase: PushBranchUseCase,
) : TabViewModel() {
    val localBranches: Flow<List<Branch>> = repositoryDataRepository.localBranches.map { it.dataOrNull().orEmpty() }
    val remoteNames: Flow<List<String>> = repositoryDataRepository.remotes.map { state ->
        state.dataOrNull()?.map { it.remote.name }.orEmpty()
    }

    fun push(
        force: Boolean,
        pushTags: Boolean,
        source: Branch?,
        remoteName: String?,
        remoteBranchName: String,
        setUpstream: Boolean,
    ) {
        val target = if (!remoteName.isNullOrBlank() && remoteBranchName.isNotBlank()) {
            Branch(
                hash = "",
                name = "refs/remotes/$remoteName/$remoteBranchName",
                isLocal = false,
            )
        } else {
            null
        }
        pushBranchUseCase(
            force = force,
            pushTags = pushTags,
            targetRemoteBranch = target,
            sourceBranch = source,
            setUpstream = setUpstream,
        )
    }
}
