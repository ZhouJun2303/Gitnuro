package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.SignOffConstants
import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.AppError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.errors.onOk
import com.zhoujun.awegit.domain.interfaces.IDoCommitGitAction
import com.zhoujun.awegit.domain.interfaces.ILoadSignOffConfigGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.Identity
import com.zhoujun.awegit.domain.models.TaskType
import kotlinx.coroutines.Deferred
import javax.inject.Inject

class DoCommitUseCase @Inject constructor(
    private val doCommitGitAction: IDoCommitGitAction,
    private val useCaseExecutor: UseCaseExecutor,
    private val loadSignOffConfigGitAction: ILoadSignOffConfigGitAction,
    private val getAuthorUseCase: GetAuthorUseCase,
    private val persistCommitMessageUseCase: PersistCommitMessageUseCase,
) {
    operator fun invoke(
        message: String,
        amend: Boolean,
        author: Identity?,
    ): Deferred<Either<Commit, AppError>> {
        return useCaseExecutor.executeLaunchAsync(
            taskType = TaskType.DoCommit,
            dataToRefresh = arrayOf(DataToRefresh.STATUS, DataToRefresh.BRANCHES, DataToRefresh.LOG, DataToRefresh.REPO_STATE),
        ) { repositoryPath ->
            val signOffConfig = loadSignOffConfigGitAction(repositoryPath).bind()

            val finalMessage = if (signOffConfig.isEnabled) {
                val authorToSign = author ?: getAuthorUseCase().bind().identityToUse()

                val signature = signOffConfig.format
                    .replace(SignOffConstants.DEFAULT_SIGN_OFF_FORMAT_USER, authorToSign.name.orEmpty())
                    .replace(SignOffConstants.DEFAULT_SIGN_OFF_FORMAT_EMAIL, authorToSign.email.orEmpty())

                "$message\n\n$signature"
            } else
                message


            doCommitGitAction(repositoryPath, finalMessage, amend, author)
                .onOk {
                    persistCommitMessageUseCase(null)
                }

        }
    }
}
