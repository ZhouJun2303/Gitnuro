package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.errors.okOrNull
import com.zhoujun.awegit.domain.models.AppConfig
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.PullOptions
import com.zhoujun.awegit.domain.models.RemoteInfo
import com.zhoujun.awegit.domain.models.TrackingBranch
import com.zhoujun.awegit.domain.repositories.DataState
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import com.zhoujun.awegit.domain.services.AppSettingsService
import com.zhoujun.awegit.domain.usecases.GetRemotesUseCase
import com.zhoujun.awegit.domain.usecases.GetTrackingBranchUseCase
import com.zhoujun.awegit.domain.usecases.PullWithOptionsUseCase
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

sealed interface PullDialogState {
    data object Loading : PullDialogState
    data object NoRemotes : PullDialogState
    data class Loaded(
        val remotes: List<RemoteInfo>,
        val selectedRemote: RemoteInfo,
        val selectedBranch: Branch?,
        val currentBranchName: String,
        val rebase: Boolean,
        val stashAndReapply: Boolean,
    ) : PullDialogState
}

class PullDialogViewModel @AssistedInject constructor(
    private val getRemotesUseCase: GetRemotesUseCase,
    private val getTrackingBranchUseCase: GetTrackingBranchUseCase,
    private val repositoryDataRepository: RepositoryDataRepository,
    private val appSettingsService: AppSettingsService,
    private val pullWithOptionsUseCase: PullWithOptionsUseCase,
    @Assisted private val preselectedRemoteBranch: Branch?,
) : TabViewModel() {
    @AssistedFactory
    interface Factory {
        fun create(preselectedRemoteBranch: Branch?): PullDialogViewModel
    }

    private val _state = MutableStateFlow<PullDialogState>(PullDialogState.Loading)
    val state: StateFlow<PullDialogState> = _state

    init {
        viewModelScope.launch { load() }
    }

    private suspend fun load() {
        val currentBranch = repositoryDataRepository.currentBranch.first { it !is DataState.Loading }.dataOrNull()
        val remotes = getRemotesUseCase().okOrNull().orEmpty()
            .map { it.copy(branchesList = it.branchesList.filter { b -> b.simpleName != "HEAD" }) }
        if (remotes.isEmpty()) {
            _state.value = PullDialogState.NoRemotes
            return
        }
        val tracking: TrackingBranch? = currentBranch?.let { getTrackingBranchUseCase(it).okOrNull() }
        val (remote, branch) = pickDefaults(remotes, currentBranch, tracking)
        _state.value = PullDialogState.Loaded(
            remotes = remotes,
            selectedRemote = remote,
            selectedBranch = branch,
            currentBranchName = currentBranch?.simpleName ?: "HEAD",
            rebase = appSettingsService.pullDialogRebase.first(),
            stashAndReapply = appSettingsService.pullDialogStashAndReapply.first(),
        )
    }

    private fun pickDefaults(
        remotes: List<RemoteInfo>,
        current: Branch?,
        tracking: TrackingBranch?,
    ): Pair<RemoteInfo, Branch?> {
        preselectedRemoteBranch?.let { pre ->
            val remote = remotes.firstOrNull { it.remote.name == pre.remoteName } ?: remotes.first()
            return remote to (remote.branchesList.firstOrNull { it.name == pre.name } ?: pre)
        }
        if (tracking != null) {
            val remote = remotes.firstOrNull { it.remote.name == tracking.remote }
            if (remote != null) return remote to remote.branchesList.firstOrNull { it.simpleName == tracking.branch }
        }
        val remote = remotes.firstOrNull { it.remote.name == "origin" } ?: remotes.first()
        return remote to (remote.branchesList.firstOrNull { it.simpleName == current?.simpleName }
            ?: remote.branchesList.firstOrNull())
    }

    fun selectRemote(remote: RemoteInfo) = updateLoaded { s ->
        s.copy(
            selectedRemote = remote,
            selectedBranch = remote.branchesList.firstOrNull { it.simpleName == s.selectedBranch?.simpleName }
                ?: remote.branchesList.firstOrNull(),
        )
    }

    fun selectBranch(branch: Branch) = updateLoaded { it.copy(selectedBranch = branch) }

    fun setRebase(value: Boolean) {
        updateLoaded { it.copy(rebase = value) }
        viewModelScope.launch { appSettingsService.setConfiguration(AppConfig.PullDialogRebase(value)) }
    }

    fun setStashAndReapply(value: Boolean) {
        updateLoaded { it.copy(stashAndReapply = value) }
        viewModelScope.launch { appSettingsService.setConfiguration(AppConfig.PullDialogStashAndReapply(value)) }
    }

    fun pull() {
        val s = _state.value as? PullDialogState.Loaded ?: return
        pullWithOptionsUseCase(PullOptions(s.selectedBranch, s.rebase, s.stashAndReapply))
    }

    private fun updateLoaded(transform: (PullDialogState.Loaded) -> PullDialogState.Loaded) {
        _state.update { if (it is PullDialogState.Loaded) transform(it) else it }
    }
}
