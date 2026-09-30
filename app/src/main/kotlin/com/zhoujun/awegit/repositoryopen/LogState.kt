package com.zhoujun.awegit.repositoryopen

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Immutable
import com.zhoujun.awegit.common.flows.combine
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.GraphCommit
import com.zhoujun.awegit.domain.models.GraphCommits
import com.zhoujun.awegit.domain.models.StatusSummary
import com.zhoujun.awegit.domain.models.Tag
import com.zhoujun.awegit.ui.UiDataState
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow

@Immutable
data class LogState(
    val isLoading: Boolean,
    val hasUncommittedChanges: Boolean = false,
    val commitList: GraphCommits = GraphCommits(),
    val currentBranch: Branch? = null,
    val branches: Map<String, List<Branch>> = emptyMap(),
    val tags: Map<String, List<Tag>> = emptyMap(),
    val stashes: HashSet<String> = HashSet(),
    val statusSummary: StatusSummary = StatusSummary(0, 0, 0, 0),
    val searchFilter: LogSearch = LogSearch.NotSearching,
    val verticalScrollState: LazyListState = LazyListState(),
    val horizontalScrollState: ScrollState = ScrollState(0),
    val unpushedCommits: Set<String> = emptySet(),
    val commits: List<GraphCommit> = commitList.commits.values.toList(),
)

fun combineLogState(
    log: StateFlow<UiDataState<GraphCommits>>,
    hasUncommittedChanges: Flow<Boolean>,
    currentBranch: StateFlow<UiDataState<Branch?>>,
    branches: Flow<Map<String, List<Branch>>>,
    tags: Flow<Map<String, List<Tag>>>,
    stashes: Flow<HashSet<String>>,
    statusSummary: Flow<StatusSummary>,
    logSearchFilterResults: Flow<LogSearch>,
    verticalListState: Flow<LazyListState>,
    horizontalListState: Flow<ScrollState>,
    unpushedCommits: Flow<Set<String>>,
): Flow<LogState> {
    return combine(
        log,
        hasUncommittedChanges,
        currentBranch,
        branches,
        tags,
        stashes,
        statusSummary,
        logSearchFilterResults,
        verticalListState,
        horizontalListState,
        unpushedCommits,
    ) { log,
        hasUncommittedChanges,
        currentBranch,
        branches,
        tags,
        stashes,
        statusSummary,
        logSearchFilterResults,
        verticalListState,
        horizontalListState,
        unpushedCommits ->
        LogState(
            isLoading = log.isLoading || currentBranch.isLoading,
            hasUncommittedChanges,
            log.data ?: GraphCommits(),
            currentBranch.data,
            branches,
            tags,
            stashes,
            statusSummary,
            logSearchFilterResults,
            verticalListState,
            horizontalListState,
            unpushedCommits,
        )
    }
}