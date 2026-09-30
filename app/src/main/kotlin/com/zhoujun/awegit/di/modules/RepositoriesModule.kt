package com.zhoujun.awegit.di.modules

import com.zhoujun.awegit.data.ai.OpenAiRepository
import com.zhoujun.awegit.data.repositories.CredentialsCacheRepository
import com.zhoujun.awegit.data.repositories.FileWorkspacesRepository
import com.zhoujun.awegit.data.repositories.InMemoryNotificationsRepository
import com.zhoujun.awegit.data.repositories.JvmSystemProxyRepository
import com.zhoujun.awegit.data.repositories.NetworkLfsRepository
import com.zhoujun.awegit.domain.repositories.AiRepository
import com.zhoujun.awegit.domain.repositories.AppSettingsRepository
import com.zhoujun.awegit.data.repositories.configuration.DataStoreAppSettingsRepository
import com.zhoujun.awegit.domain.repositories.CredentialsRepository
import com.zhoujun.awegit.domain.repositories.LfsRepository
import com.zhoujun.awegit.domain.repositories.NotificationsRepository
import com.zhoujun.awegit.domain.repositories.SystemProxyRepository
import com.zhoujun.awegit.domain.repositories.WorkspacesRepository
import dagger.Binds
import dagger.Module
import javax.inject.Singleton

@Module
interface RepositoriesModule {
    @Binds
    fun notificationsRepository(repository: InMemoryNotificationsRepository): NotificationsRepository

    @Binds
    fun systemProxyRepository(repository: JvmSystemProxyRepository): SystemProxyRepository

    @Singleton
    @Binds
    fun appSettingsRepository(repository: DataStoreAppSettingsRepository): AppSettingsRepository

    @Singleton
    @Binds
    fun credentialsRepository(repository: CredentialsCacheRepository): CredentialsRepository

    @Binds
    fun lfsRepository(repository: NetworkLfsRepository): LfsRepository

    @Singleton
    @Binds
    fun aiRepository(repository: OpenAiRepository): AiRepository

    @Singleton
    @Binds
    fun workspacesRepository(repository: FileWorkspacesRepository): WorkspacesRepository
}