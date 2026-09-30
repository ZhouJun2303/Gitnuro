package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.SignOffConstants
import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.interfaces.IDoCommitGitAction
import com.zhoujun.awegit.domain.interfaces.ILoadSignOffConfigGitAction
import com.zhoujun.awegit.domain.interfaces.IPushBranchGitAction
import com.zhoujun.awegit.domain.models.Identity
import com.zhoujun.awegit.domain.models.TaskType
import com.zhoujun.awegit.domain.services.AppSettingsService
import javax.inject.Inject
import kotlinx.coroutines.flow.first

class CommitAndPushUseCase @Inject constructor(
    private val doCommitGitAction: IDoCommitGitAction,
    private val pushBranchGitAction: IPushBranchGitAction,
    private val loadSignOffConfigGitAction: ILoadSignOffConfigGitAction,
    private val getAuthorUseCase: GetAuthorUseCase,
    private val persistCommitMessageUseCase: PersistCommitMessageUseCase,
    private val appSettingsService: AppSettingsService,
    private val useCaseExecutor: UseCaseExecutor,
) {
    operator fun invoke(message: String, amend: Boolean, author: Identity?) =
        useCaseExecutor.executeLaunch(
            taskType = TaskType.CommitAndPush,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            val signOffConfig = loadSignOffConfigGitAction(repositoryPath).bind()
            val finalMessage = if (signOffConfig.isEnabled) {
                val authorToSign = author ?: getAuthorUseCase().bind().identityToUse()
                val signature = signOffConfig.format
                    .replace(SignOffConstants.DEFAULT_SIGN_OFF_FORMAT_USER, authorToSign.name.orEmpty())
                    .replace(SignOffConstants.DEFAULT_SIGN_OFF_FORMAT_EMAIL, authorToSign.email.orEmpty())
                "$message\n\n$signature"
            } else {
                message
            }
            doCommitGitAction(repositoryPath, finalMessage, amend, author).bind()
            persistCommitMessageUseCase(null)
            val pushWithLease = appSettingsService.pushWithLease.first()
            pushBranchGitAction(repositoryPath, force = false, pushTags = false, pushWithLease = pushWithLease, specificBranch = null)
        }
}
