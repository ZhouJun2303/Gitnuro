package com.zhoujun.awegit.domain.models

data class DiffText(
    val text: String,
    val files: List<String>,
    val truncated: Boolean,
)
