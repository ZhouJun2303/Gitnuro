package com.zhoujun.awegit.data.git.tags

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.git.remote_operations.HandleTransportGitAction
import com.zhoujun.awegit.data.git.signers.AppGpgSigner
import com.zhoujun.awegit.data.git.signers.SshSigner
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.interfaces.ICreateTagGitAction
import com.zhoujun.awegit.domain.models.Commit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.ObjectId
import org.eclipse.jgit.revwalk.RevCommit
import org.eclipse.jgit.revwalk.RevWalk
import javax.inject.Inject
import javax.inject.Provider

private const val GPG_FORMAT_SSH = "ssh"
private const val GPG_FORMAT_GPG = "gpg"

class CreateTagGitAction @Inject constructor(
    private val jgit: JGit,
    private val handleTransportGitAction: HandleTransportGitAction,
    private val sshSigner: Provider<SshSigner>,
    private val gpgSigner: Provider<AppGpgSigner>,
) : ICreateTagGitAction {
    override suspend operator fun invoke(
        repositoryPath: String,
        tag: String,
        commit: Commit,
        message: String?,
        pushToOrigin: Boolean,
    ) =
        jgit.provide(repositoryPath) { git ->
            val commitId =
                ObjectId.fromString(commit.hash) // TODO Should this be used instead of "git.repository.resolve(revCommit.hash) ?: throw Exception("Commit ${revCommit.hash} not found")" used in other places?
            val parsedCommit: RevCommit? = RevWalk(git.repository).use { revWalk ->
                revWalk.parseCommit(commitId)
            }
            val annotated = message == null || message.isNotBlank()

            git
                .tag()
                .setAnnotated(annotated)
                .setName(tag)
                .setObjectId(parsedCommit)
                .apply {
                    if (!message.isNullOrBlank()) {
                        setMessage(message)
                    }
                }
                .run {
                    if (!annotated) {
                        return@run this
                    }
                    val signConfig = getTagSigningConfig(git)

                    if (signConfig.isSigningEnabled && signConfig.signingType != null && signConfig.signingKey != null) {
                        val signer = when (signConfig.signingType) {
                            SigningType.SSH -> sshSigner.get()
                            SigningType.GPG -> gpgSigner.get()
                        }

                        setSigned(true)
                            .setSigner(signer)
                            .setSigningKey(signConfig.signingKey)

                    } else {
                        setSigned(false)
                    }
                }
                .call()

            if (pushToOrigin) {
                handleTransportGitAction(repositoryPath) {
                    withContext(Dispatchers.IO) {
                        git.push()
                            .setRemote("origin")
                            .setPushTags()
                            .setTransportConfigCallback { handleTransport(it) }
                            .call()
                    }
                }.bind()
            }

            Unit
        }

    private fun getTagSigningConfig(git: Git): TagSignConfig {
        val config = git.repository.config.apply {
            load()
        }

        val signTag = config.getBoolean("tag", null, "gpgSign") ?: false
        val gpgFormat: String? = config.getString("gpg", null, "format")
        val signingKey: String? = config.getString("user", null, "signingkey")

        return if (signTag) {
            val type = when (gpgFormat) {
                GPG_FORMAT_GPG -> SigningType.GPG
                GPG_FORMAT_SSH -> SigningType.SSH
                else -> throw IllegalStateException("Unsupported gpg format: $gpgFormat")
            }

            if (signingKey == null) {
                throw IllegalStateException("Can't sign tags. Signing key not set: $signingKey")
            }

            TagSignConfig(signTag, type, signingKey)
        } else {
            TagSignConfig(isSigningEnabled = false, signingType = null, signingKey = null)
        }
    }
}

private data class TagSignConfig(
    val isSigningEnabled: Boolean,
    val signingType: SigningType?,
    val signingKey: String?,
)

private enum class SigningType {
    SSH,
    GPG,
}