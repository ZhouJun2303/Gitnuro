package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IGetTrackingBranchGitAction
import com.zhoujun.awegit.domain.models.Branch
import javax.inject.Inject

class GetTrackingBranchUseCase @Inject constructor(
    private val getTrackingBranchGitAction: IGetTrackingBranchGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend operator fun invoke(branch: Branch) = useCaseExecutor.execute { repositoryPath ->
        getTrackingBranchGitAction(repositoryPath, branch)
    }
}