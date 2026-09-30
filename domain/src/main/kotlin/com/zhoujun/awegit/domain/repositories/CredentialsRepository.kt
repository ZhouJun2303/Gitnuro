package com.zhoujun.awegit.domain.repositories

import com.zhoujun.awegit.domain.models.CredentialsType

interface CredentialsRepository {
    fun getCachedHttpCredentials(url: String, isLfs: Boolean): CredentialsType.HttpCredentials?
    fun getCachedSshCredentials(url: String): CredentialsType.SshCredentials?

    suspend fun cacheHttpCredentials(credentials: CredentialsType.HttpCredentials)

    suspend fun cacheHttpCredentials(url: String, userName: String, password: String, isLfs: Boolean)

    suspend fun cacheSshCredentials(url: String, password: String)
}