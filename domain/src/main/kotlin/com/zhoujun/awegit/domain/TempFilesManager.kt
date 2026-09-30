package com.zhoujun.awegit.domain

import com.zhoujun.awegit.common.AppDirectories
import com.zhoujun.awegit.domain.extensions.openDirectory
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class TempFilesManager @Inject constructor(
    private val appFilesManager: AppFilesManager,
) {
    fun tempDir(): File {
        val appDataDir = appFilesManager.getAppFolder()
        return appDataDir.openDirectory("tmp")
    }

    fun clearAll() {
        val dir = tempDir()
        dir.deleteRecursively()
    }
}

class AppFilesManager @Inject constructor() {
    fun getAppFolder(): File = AppDirectories.configDir()
}