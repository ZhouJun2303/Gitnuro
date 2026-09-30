package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.models.RepositorySelectionState
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import javax.inject.Inject

class SetRepositorySelectionStateToNoneUseCase @Inject constructor(
    private val repositoryDataRepository: RepositoryDataRepository,
) {
    operator fun invoke() {
        repositoryDataRepository.setRepositorySelectionState(RepositorySelectionState.None)
    }
}