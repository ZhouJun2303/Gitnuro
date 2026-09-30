package com.zhoujun.awegit.domain.credentials.external

interface IGitCredentialsManagerProvider {
    fun loadPath(): String?
}