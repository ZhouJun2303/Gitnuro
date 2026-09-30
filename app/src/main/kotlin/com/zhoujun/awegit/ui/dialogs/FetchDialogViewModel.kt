package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.models.Remote
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import com.zhoujun.awegit.domain.usecases.FetchWithOptionsUseCase
import javax.inject.Inject
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

class FetchDialogViewModel @Inject constructor(
    repositoryDataRepository: RepositoryDataRepository,
    private val fetchWithOptionsUseCase: FetchWithOptionsUseCase,
) : TabViewModel() {
    val remotes: Flow<List<Remote>> = repositoryDataRepository.remotes.map { state ->
        state.dataOrNull()?.map { it.remote }.orEmpty()
    }

    fun fetch(remote: Remote?, fetchAllTags: Boolean, prune: Boolean) {
        fetchWithOptionsUseCase(remote, fetchAllTags, prune)
    }
}
