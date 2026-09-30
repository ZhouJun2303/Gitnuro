package com.zhoujun.awegit.data.di

import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import com.zhoujun.awegit.data.UserSettingsDataStore
import com.zhoujun.awegit.data.repositories.configuration.getPreferencesPath
import dagger.Module
import dagger.Provides
import okio.Path.Companion.toPath
import javax.inject.Singleton

@Module
class DatastoreModule {
    @Singleton
    @Provides
    fun provideDataStore(): UserSettingsDataStore {
        val preferences = PreferenceDataStoreFactory.createWithPath(
            produceFile = { getPreferencesPath().toPath() }
        )

        return UserSettingsDataStore(preferences)
    }
}