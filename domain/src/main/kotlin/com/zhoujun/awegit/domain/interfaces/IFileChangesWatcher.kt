package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.FSWatchError
import com.zhoujun.awegit.domain.models.WatcherEvent
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharedFlow
import org.eclipse.jgit.lib.Repository

interface IFileChangesWatcher {
    fun addPathToWatch(path: String, isRecursive: Boolean)
    fun removePathFromWatch(path: String)

    suspend fun observeEvents(): Flow<WatcherEvent>

    fun close()
}