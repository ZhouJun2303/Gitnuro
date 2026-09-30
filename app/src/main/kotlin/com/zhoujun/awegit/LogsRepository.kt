package com.zhoujun.awegit

import com.zhoujun.awegit.common.AppDirectories
import org.apache.log4j.*
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class LogsRepository @Inject constructor() {
    val logsDirectory: File = AppDirectories.logsDir()

    fun initLogging() {
        val layout = PatternLayout("%d{yyyy-MM-dd HH:mm:ss} %-5p %c{1}:%L - %m%n")

        val filePath = logsFile()

        val fileAppender = RollingFileAppender(layout, filePath, true)
        fileAppender.maximumFileSize = 10 * 1024 * 1024 // 10MB
        fileAppender.maxBackupIndex = 5

        val consoleAppender = ConsoleAppender(layout)

        LogManager.getRootLogger().apply {
            addAppender(fileAppender)
            addAppender(consoleAppender)
            level = Level.INFO
        }
    }

    private fun logsFile(): String {
        val file = File(logsDirectory, "awegit.log")

        return file.absolutePath
    }
}
