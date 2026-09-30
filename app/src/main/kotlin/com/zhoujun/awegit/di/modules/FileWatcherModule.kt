package com.zhoujun.awegit.di.modules

import com.zhoujun.awegit.data.git.FileChangesWatcher
import com.zhoujun.awegit.domain.interfaces.IFileChangesWatcher
import dagger.Binds
import dagger.Module

@Module
interface FileWatcherModule {
    @Binds
    fun bindFileWatcher(watcher: FileChangesWatcher) : IFileChangesWatcher
}