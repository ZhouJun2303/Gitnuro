package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IGetFileCommitsAction
import javax.inject.Inject

class GetFileCommitsUseCase @Inject constructor(
    private val getFileCommitsAction: IGetFileCommitsAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend operator fun invoke(filePath: String) = useCaseExecutor.execute(
    ) { repositoryPath ->
        getFileCommitsAction(repositoryPath, filePath)
    }
}