package com.zhoujun.awegit.data.git.log

import org.eclipse.jgit.lib.GpgConfig
import org.eclipse.jgit.lib.Repository
import org.eclipse.jgit.lib.Signers
import org.eclipse.jgit.transport.CredentialsProvider

object JGitCommitSigner {
    fun fromConfig(repository: Repository): CommitSigner? {
        val config = GpgConfig(repository.config)
        if (!config.isSignCommits) return null
        val signer = Signers.get(config.keyFormat)
            ?: throw IllegalStateException("No signer registered for ${config.keyFormat}")
        return CommitSigner { builder, committer ->
            signer.signObject(repository, config, builder, committer, config.signingKey, CredentialsProvider.getDefault())
        }
    }
}
