package com.zhoujun.awegit.domain.credentials.external

import com.zhoujun.awegit.domain.ShellManager
import javax.inject.Inject

class NixGitCredentialsManagerProvider @Inject constructor(
    private val shellManager: ShellManager,
) : IGitCredentialsManagerProvider {
    override fun loadPath(): String? {
        return listOf(
            "git-credential-manager",
            "git-credential-manager-core"
        ).firstOrNull { program ->
            val checkInstalled = shellManager.runCommand(listOf("which", program, "2>/dev/null"))
            !checkInstalled.isNullOrEmpty()
        }
    }
}