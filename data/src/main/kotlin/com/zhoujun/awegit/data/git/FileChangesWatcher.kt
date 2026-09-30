package com.zhoujun.awegit.data.git

import com.zhoujun.awegit.FileChanged
import com.zhoujun.awegit.FileWatcher
import com.zhoujun.awegit.WatchDirectoryNotifier
import com.zhoujun.awegit.common.TabScope
import com.zhoujun.awegit.domain.interfaces.IFileChangesWatcher
import com.zhoujun.awegit.domain.models.WatcherEvent
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.channels.trySendBlocking
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.isActive
import javax.inject.Inject

private const val TAG = "FileChangesWatcher"

@TabScope
class FileChangesWatcher @Inject constructor() : AutoCloseable, IFileChangesWatcher {
    private val fileWatcher = FileWatcher()
    private var shouldKeepLooping = true

    init {
        // TODO add error handling
        fileWatcher.init()
    }

    override fun addPathToWatch(path: String, isRecursive: Boolean) {
        fileWatcher.addWatch(path, isRecursive)
    }

    override fun removePathFromWatch(path: String) {
        fileWatcher.removeWatch(path)
    }

    override suspend fun observeEvents(): Flow<WatcherEvent> = callbackFlow {
        fileWatcher.watch(
            notifier = object : WatchDirectoryNotifier {
                override fun shouldKeepLooping(): Boolean = coroutineContext.isActive && shouldKeepLooping
                override fun detectedChange(paths: List<FileChanged>) {
                    trySendBlocking(WatcherEvent.ChangesDetected(paths))
                }

                override fun onError(code: Int) {
                    trySendBlocking(WatcherEvent.WatchInitError(code))
                }
            }
        )

        awaitClose { fileWatcher.close() }
    }


    override fun close() {
        shouldKeepLooping = false
        fileWatcher.close()
    }
}
