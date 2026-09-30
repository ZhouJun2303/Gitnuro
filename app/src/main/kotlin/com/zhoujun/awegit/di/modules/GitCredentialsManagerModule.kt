package com.zhoujun.awegit.di.modules

import com.zhoujun.awegit.common.OS
import com.zhoujun.awegit.common.currentOs
import com.zhoujun.awegit.domain.credentials.external.IGitCredentialsManagerProvider
import com.zhoujun.awegit.domain.credentials.external.NixGitCredentialsManagerProvider
import com.zhoujun.awegit.domain.credentials.external.WindowsGitCredentialsManagerProvider
import dagger.Module
import dagger.Provides
import javax.inject.Provider

@Module
class GitCredentialsManagerModule {
    @Provides
    fun providesGitCredentialsManagerProvider(
        windowsGitCredentialsManagerProvider: Provider<WindowsGitCredentialsManagerProvider>,
        nixGitCredentialsManagerProvider: Provider<NixGitCredentialsManagerProvider>,
    ): IGitCredentialsManagerProvider {
        return when (currentOs) {
            OS.LINUX, OS.MAC ->  nixGitCredentialsManagerProvider.get() // TODO Test this on MacOs
            OS.WINDOWS -> windowsGitCredentialsManagerProvider.get()
            OS.UNKNOWN -> throw IllegalStateException("Unknown OS")
        }
    }
}