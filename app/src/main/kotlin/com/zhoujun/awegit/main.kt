package com.zhoujun.awegit

import com.zhoujun.awegit.common.AppDirectories
import com.zhoujun.awegit.data.repositories.configuration.initPreferencesPath
import com.zhoujun.awegit.data.repositories.configuration.migrateLegacyPreferencesNode
import com.zhoujun.awegit.di.DaggerAppComponent
import org.bouncycastle.jce.provider.BouncyCastleProvider
import java.security.Security


suspend fun main(args: Array<String>) {
    if (args.contains("--graalvm")) {
        val currentDir = System.getProperty("user.dir")

        System.setProperty("java.home", currentDir)
    }

    Security.addProvider(BouncyCastleProvider())

    AppDirectories.migrateLegacyFilesIfNeeded()
    initPreferencesPath()
    migrateLegacyPreferencesNode()

    val app: App = DaggerAppComponent
        .create()
        .app()

    app.start(args)
}
