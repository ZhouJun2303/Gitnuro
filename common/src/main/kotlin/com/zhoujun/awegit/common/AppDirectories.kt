package com.zhoujun.awegit.common

import java.io.File

object AppDirectories {
    const val APP_DIR_NAME = "awegit"
    const val LEGACY_APP_DIR_NAME = "gitnuro"
    const val PREFERENCES_FILE_NAME = "user_prefs.preferences_pb"

    private val home: String get() = System.getProperty("user.home").orEmpty()

    private fun configBase(): File = when (currentOs) {
        OS.LINUX -> File(System.getenv("XDG_CONFIG_HOME").takeUnless { it.isNullOrBlank() } ?: "$home/.config")
        OS.MAC -> File(home, "Library/Application Support")
        OS.WINDOWS -> File(System.getenv("APPDATA").takeUnless { it.isNullOrBlank() } ?: "$home/AppData/Roaming")
        else -> File(home)
    }

    /** 设置、workspaces.json、缓存、tmp、原生库 */
    fun configDir(): File = File(configBase(), APP_DIR_NAME).apply { mkdirs() }

    fun logsDir(): File = when (currentOs) {
        OS.LINUX -> File(System.getenv("XDG_STATE_HOME").takeUnless { it.isNullOrBlank() } ?: "$home/.local/state", "$APP_DIR_NAME/logs")
        OS.MAC -> File(home, "Library/Logs/com.zhoujun.awegit")
        OS.WINDOWS -> File(System.getenv("LOCALAPPDATA").takeUnless { it.isNullOrBlank() } ?: "$home/AppData/Local", "AweGit/logs")
        else -> File(home, "$APP_DIR_NAME/logs")
    }.apply { mkdirs() }

    fun preferencesFile(): File = File(configDir(), PREFERENCES_FILE_NAME)

    private fun legacyConfigDirs(): List<File> = when (currentOs) {
        OS.LINUX -> listOfNotNull(
            System.getenv("XDG_CONFIG_HOME")?.takeIf { it.isNotBlank() }?.let { File(it, LEGACY_APP_DIR_NAME) },
            File(home, ".config/$LEGACY_APP_DIR_NAME"),
        )
        OS.MAC -> listOf(File(home, "Library/Application Support/$LEGACY_APP_DIR_NAME"))
        else -> listOf(File(home, LEGACY_APP_DIR_NAME))
    }

    /** 新位置没有数据时，从 Gitnuro 的位置复制一次；旧数据不删除 */
    fun migrateLegacyFilesIfNeeded() {
        val target = preferencesFile()
        if (!target.exists()) {
            legacyConfigDirs().map { File(it, PREFERENCES_FILE_NAME) }.firstOrNull { it.exists() }?.copyTo(target)
        }
        if (currentOs == OS.LINUX) {
            // FileSystemPreferences 存在 <userRoot>/.java/.userPrefs
            val newJava = File(configDir(), ".java")
            val oldJava = legacyConfigDirs().map { File(it, ".java") }.firstOrNull { it.exists() }
            if (!newJava.exists() && oldJava != null) oldJava.copyRecursively(newJava)
        }
    }
}
