package com.zhoujun.awegit.domain.models

enum class GitFlowBranchType { Feature, Release, Hotfix, Support }

data class GitFlowConfig(
    val master: String = "main",
    val develop: String = "develop",
    val featurePrefix: String = "feature/",
    val releasePrefix: String = "release/",
    val hotfixPrefix: String = "hotfix/",
    val supportPrefix: String = "support/",
    val versionTagPrefix: String = "",
)
