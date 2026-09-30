package com.zhoujun.awegit.repositoryopen

import com.zhoujun.awegit.domain.models.*

sealed interface LogSearch {
    data object NotSearching : LogSearch
    data class SearchResults(
        val commits: List<GraphCommit>,
        val index: Int,
        val totalCount: Int = commits.count(),
    ) : LogSearch {
        val hashes: Set<String> = commits.mapTo(HashSet()) { it.hash }
    }
}


