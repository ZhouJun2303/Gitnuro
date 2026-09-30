package com.zhoujun.awegit.di

import com.zhoujun.awegit.App
import com.zhoujun.awegit.data.di.DatastoreModule
import com.zhoujun.awegit.di.modules.*
import dagger.Component
import javax.inject.Singleton

@Singleton
@Component(
    modules = [
        ShellModule::class,
        NetworkModule::class,
        GitCredentialsManagerModule::class,
        RepositoriesModule::class,
        DatastoreModule::class,
    ]
)
interface AppComponent {
    fun app(): App
    fun tabComponentFactory(): TabComponent.Factory
}
