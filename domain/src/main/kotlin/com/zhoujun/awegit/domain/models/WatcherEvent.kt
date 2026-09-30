package com.zhoujun.awegit.domain.models

import com.zhoujun.awegit.FileChanged

sealed interface WatcherEvent {
    data class WatchInitError(val code: Int) : WatcherEvent
    data class ChangesDetected(val changes: List<FileChanged>) : WatcherEvent
}