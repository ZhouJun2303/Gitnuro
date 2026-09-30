package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.ILoadAuthorGitAction
import javax.inject.Inject

class GetAuthorUseCase @Inject constructor(
    private val loadAuthorGitAction: ILoadAuthorGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend operator fun invoke() = useCaseExecutor.execute() { repositoryPath ->
        loadAuthorGitAction(repositoryPath)
    }
}