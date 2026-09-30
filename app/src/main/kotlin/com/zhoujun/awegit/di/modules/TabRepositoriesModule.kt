package com.zhoujun.awegit.di.modules

import com.zhoujun.awegit.common.TabScope
import com.zhoujun.awegit.data.repositories.InMemoryRepositoryDataRepository
import com.zhoujun.awegit.data.repositories.InMemoryRepositoryStateRepository
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.RepositoryStateRepository
import dagger.Binds
import dagger.Module

@Module
interface TabRepositoriesModule {
    @TabScope
    @Binds
    fun statusRepository(repository: InMemoryRepositoryDataRepository): RepositoryDataRepository

    @TabScope
    @Binds
    fun repositoryStateRepository(repository: InMemoryRepositoryStateRepository): RepositoryStateRepository
}