package com.zhoujun.awegit.domain.conflicts

data class ConflictSides(
    val mine: String,
    val theirs: String,
    val workingTree: String,
)
