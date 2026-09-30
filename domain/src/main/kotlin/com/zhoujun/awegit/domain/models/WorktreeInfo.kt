package com.zhoujun.awegit.domain.models

data class WorktreeInfo(
    val path: String,
    val branch: String,
)

data class WorktreeListResult(
    val gitAvailable: Boolean,
    val worktrees: List<WorktreeInfo>,
)
