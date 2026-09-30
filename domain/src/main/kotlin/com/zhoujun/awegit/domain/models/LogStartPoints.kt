package com.zhoujun.awegit.domain.models

data class LogStartPoints(
    val hashes: List<String>,
    val stashes: Set<String>,
    val headHash: String?,
)
