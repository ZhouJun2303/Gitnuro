package com.zhoujun.awegit.domain.models

data class CustomCommand(
    val id: String,
    val name: String,
    val target: CustomCommandTarget,
    val command: String,
    val showOutput: Boolean = true,
    val confirm: Boolean = false,
)

enum class CustomCommandTarget { REPOSITORY, COMMIT, BRANCH, FILE }
