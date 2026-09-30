package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.common.measureAndLog
import com.zhoujun.awegit.domain.Pagination
import com.zhoujun.awegit.domain.RebaseConstants
import com.zhoujun.awegit.domain.TabCoroutineScope
import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.LogGenerationSupersededError
import com.zhoujun.awegit.domain.errors.flatten
import com.zhoujun.awegit.domain.errors.mapOk
import com.zhoujun.awegit.domain.errors.onOk
import com.zhoujun.awegit.domain.interfaces.*
import com.zhoujun.awegit.domain.models.RebaseInteractiveState
import com.zhoujun.awegit.domain.models.RepositoryState
import com.zhoujun.awegit.domain.repositories.DataState
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.RepositoryStateRepository
import kotlinx.coroutines.launch
import org.eclipse.jgit.api.RebaseCommand
import java.io.File
import javax.inject.Inject

class RefreshDataUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val getBranchesGitAction: IGetBranchesGitAction,
    private val getCurrentBranchGitAction: IGetCurrentBranchGitAction,
    private val repositoryDataRepository: RepositoryDataRepository,
    private val repositoryStateRepository: RepositoryStateRepository,
    private val getStashListGitAction: IGetStashListGitAction,
    private val loadAuthorGitAction: ILoadAuthorGitAction,
    private val getStatusGitAction: IGetStatusGitAction,
    private val getRemotesUseCase: GetRemotesUseCase,
    private val getSubmodulesGitAction: IGetSubmodulesGitAction,
    private val getTagsGitAction: IGetTagsGitAction,
    private val getRepositoryState: IGetRepositoryStateGitAction,
    private val getRebaseInteractiveTodoLinesUseCase: GetRebaseInteractiveTodoLinesUseCase,
    private val getRebaseLinesFullMessageUseCase: GetRebaseLinesFullMessageUseCase,
    private val getPersistedCommitMessagesGitAction: IGetPersistedCommitMessagesGitAction,
    private val getLogUseCase: GetLogUseCase,
    private val getUnpushedCommitsGitAction: IGetUnpushedCommitsGitAction,
    private val getBranchesTrackingStatusGitAction: IGetBranchesTrackingStatusGitAction,
    private val scope: TabCoroutineScope,
) {
    operator fun invoke(vararg dataToRefresh: DataToRefresh) = scope.launch {
        val isRefreshAll = dataToRefresh.contains(DataToRefresh.ALL)
        val requestedLog = isRefreshAll || dataToRefresh.contains(DataToRefresh.LOG)
        val refreshingStatus = isRefreshAll || dataToRefresh.contains(DataToRefresh.STATUS)
        val hadChanges = if (refreshingStatus) {
            repositoryDataRepository.latestStatus?.let { it.staged.isNotEmpty() || it.unstaged.isNotEmpty() }
        } else {
            null
        }

        repositoryStateRepository.refreshTriggered(dataToRefresh.toList())

        if (isRefreshAll || dataToRefresh.contains(DataToRefresh.BRANCHES)) {
            refreshBranches()
        }

        if (isRefreshAll || dataToRefresh.contains(DataToRefresh.STASHES)) {
            refreshStashes()
        }

        if (isRefreshAll || dataToRefresh.contains(DataToRefresh.STATUS)) {
            refreshStatus()
            refreshCommitMessages()
        }

        if (isRefreshAll || dataToRefresh.contains(DataToRefresh.GIT_CONFIG)) {
            refreshGitConfig()
        }

        if (isRefreshAll || dataToRefresh.contains(DataToRefresh.REMOTES)) {
            refreshRemotes()
        }

        if (isRefreshAll || dataToRefresh.contains(DataToRefresh.SUBMODULES)) {
            refreshSubmodules()
        }

        if (isRefreshAll || dataToRefresh.contains(DataToRefresh.TAGS)) {
            refreshTags()
        }

        if (isRefreshAll || dataToRefresh.contains(DataToRefresh.REPO_STATE)) {
            refreshRepositoryState()
        }

        if (requestedLog) {
            refreshLog()
        } else if (hadChanges != null) {
            val hasChanges = repositoryDataRepository.latestStatus?.let { it.staged.isNotEmpty() || it.unstaged.isNotEmpty() } ?: false
            if (hadChanges != hasChanges) refreshLog()
        }
    }

    private suspend fun refreshCommitMessages() {
        useCaseExecutor.executeWithoutResult { repositoryPath ->
            repositoryDataRepository.updatePersistedCommitMessages {
                getPersistedCommitMessagesGitAction(repositoryPath)
            }
        }
    }

    private suspend fun refreshBranches() {
        useCaseExecutor.executeWithoutResult { repositoryPath ->
            repositoryDataRepository.updateLocalBranches {
                getBranchesGitAction(repositoryPath)
            }

            repositoryDataRepository.updateCurrentBranch {
                getCurrentBranchGitAction(repositoryPath)
            }
            getBranchesTrackingStatusGitAction(repositoryPath).onOk {
                repositoryDataRepository.updateBranchesTracking(it)
            }
        }
    }

    private suspend fun refreshStashes() {
        useCaseExecutor.executeWithoutResult { repositoryPath ->
            repositoryDataRepository.updateStashes { getStashListGitAction(repositoryPath) }
        }
    }

    private suspend fun refreshLog() {
        measureAndLog("RefreshDataUseCase", "refreshLog") {
            useCaseExecutor.executeWithoutResult { repositoryPath ->
                val result = getLogUseCase(repositoryPath, pagination = Pagination.None)
                if (result is Either.Err && result.error is LogGenerationSupersededError) return@executeWithoutResult
                repositoryDataRepository.updateLog { result }
                getUnpushedCommitsGitAction(repositoryPath).onOk { repositoryDataRepository.updateUnpushedCommits(it) }
            }
        }
    }

    private suspend fun refreshStatus() {
        measureAndLog("RefreshDataUseCase", "refreshStatus") {
            useCaseExecutor.executeWithoutResult() { repositoryPath ->
                repositoryDataRepository.updateStatus {
                    getStatusGitAction(repositoryPath)
                }
            }
        }
    }

    private suspend fun refreshGitConfig() {
        useCaseExecutor.executeWithoutResult() { repositoryPath ->
            repositoryDataRepository.updateAuthor {
                loadAuthorGitAction(repositoryPath)
            }
        }
    }

    private suspend fun refreshRemotes() {
        useCaseExecutor.executeWithoutResult() { repositoryPath ->
            repositoryDataRepository.updateRemotes { getRemotesUseCase() }
        }
    }

    private suspend fun refreshSubmodules() {
        useCaseExecutor.executeWithoutResult() { repositoryPath ->
            repositoryDataRepository.updateSubmodules {
                getSubmodulesGitAction(repositoryPath)
            }
        }
    }

    private suspend fun refreshTags() {
        useCaseExecutor.executeWithoutResult() { repositoryPath ->
            repositoryDataRepository.updateTags {
                getTagsGitAction(repositoryPath)
            }
        }
    }

    private suspend fun refreshRepositoryState() {
        useCaseExecutor.executeWithoutResult() { repositoryPath ->
            repositoryDataRepository.updateRepositoryState {
                getRepositoryState(repositoryPath)
            }

            val state = (repositoryDataRepository.repositoryState.value as? DataState.Loaded<RepositoryState>)?.data

            if (state == RepositoryState.REBASING_INTERACTIVE) {
                // TODO Error local handling or keep as it is?

                // TODO is this check necessary with this newer arch?
//            val isSameRebase = isSameRebase(rebaseLines, _rebaseState.value)

//            if (!isSameRebase) {
//                return@either Either.Ok(RebaseInteractiveViewState.Loaded(rebaseLines, messages))
//                val firstLine = rebaseLines.firstOrNull()
// TODO Check what is this logic for and if still necessary
//                if (firstLine != null) {
//                    val fullCommit = getCommitFromRebaseLineUseCase(firstLine.commit, firstLine.shortMessage)
//                    tabState.newSelectedCommit(fullCommit)
//                }
//            }

                repositoryDataRepository.updateRebaseInteractiveState {
                    val rebaseMergeDir = File(repositoryPath, RebaseConstants.REBASE_MERGE)
                    val doneFile = File(rebaseMergeDir, RebaseConstants.DONE)
                    val stoppedShaFile = File(rebaseMergeDir, RebaseConstants.STOPPED_SHA)

                    val newState = when {
                        !rebaseMergeDir.exists() -> RebaseInteractiveState.None
                        doneFile.exists() || stoppedShaFile.exists() -> {
                            val commitId: String? = getRebaseAmendCommitId(repositoryPath)

                            RebaseInteractiveState.ProcessingCommits(commitId)
                        }

                        else -> {
                            val lines = getRebaseInteractiveTodoLinesUseCase()
                                .mapOk { originalLines ->
                                    getRebaseLinesFullMessageUseCase(originalLines)
                                }
                                .flatten()

                            when (lines) {
                                is Either.Err -> return@updateRebaseInteractiveState lines
                                is Either.Ok -> RebaseInteractiveState.AwaitingInteraction(lines.value)
                            }
                        }
                    }

                    Either.Ok(newState)
                }
            } else {
                repositoryDataRepository.updateRebaseInteractiveState {
                    Either.Ok(RebaseInteractiveState.None)
                }
            }
        }
    }

    private fun getRebaseAmendCommitId(repository: String): String? {
        val amendFile = File(repository, "${RebaseCommand.REBASE_MERGE}/${RebaseConstants.AMEND}")

        return if (!amendFile.exists()) {
            null
        } else {
            amendFile.readText().removeSuffix("\n").removeSuffix("\r\n")
        }
    }
}

enum class DataToRefresh {
    ALL,
    BRANCHES,
    GIT_CONFIG,
    LOG,
    REMOTES,
    REPO_STATE,
    STASHES,
    STATUS,
    SUBMODULES,
    TAGS,
}