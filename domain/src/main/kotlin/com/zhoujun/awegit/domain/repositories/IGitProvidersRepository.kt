package com.zhoujun.awegit.domain.repositories

// TODO do we want to use this?
interface IGitProvidersRepository {
    fun initialize()
    fun cleanup()
}