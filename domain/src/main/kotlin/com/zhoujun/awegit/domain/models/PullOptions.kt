package com.zhoujun.awegit.domain.models

data class PullOptions(
    val remoteBranch: Branch?,
    val rebase: Boolean,
    val stashAndReapply: Boolean,
)
