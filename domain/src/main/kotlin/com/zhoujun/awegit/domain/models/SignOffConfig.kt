package com.zhoujun.awegit.domain.models

data class SignOffConfig(
    val isEnabled: Boolean,
    val format: String,
    val hiddenRefs: List<String> = emptyList(),
)
