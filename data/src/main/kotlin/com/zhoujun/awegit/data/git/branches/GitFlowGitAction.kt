package com.zhoujun.awegit.data.git.branches

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IGitFlowGitAction
import com.zhoujun.awegit.domain.models.GitFlowBranchType
import com.zhoujun.awegit.domain.models.GitFlowConfig
import javax.inject.Inject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.MergeCommand
import org.eclipse.jgit.lib.Constants

class GitFlowGitAction @Inject constructor(
    private val jgit: JGit,
) : IGitFlowGitAction {
    override suspend fun load(repositoryPath: String) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            readConfig(git.repository.config)
        }
    }

    override suspend fun save(repositoryPath: String, config: GitFlowConfig) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            writeConfig(git.repository.config, config)
        }
    }

    override suspend fun init(repositoryPath: String, config: GitFlowConfig) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            writeConfig(git.repository.config, config)
            val develop = git.repository.findRef(Constants.R_HEADS + config.develop)
            if (develop == null) {
                git.branchCreate()
                    .setName(config.develop)
                    .setStartPoint(config.master)
                    .call()
            }
        }
    }

    override suspend fun start(repositoryPath: String, type: GitFlowBranchType, name: String) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            val config = readConfig(git.repository.config)
            val base = if (type == GitFlowBranchType.Hotfix) config.master else config.develop
            val branch = prefix(config, type) + name
            git.checkout()
                .setCreateBranch(true)
                .setName(branch)
                .setStartPoint(base)
                .call()
        }
    }

    override suspend fun finish(
        repositoryPath: String,
        type: GitFlowBranchType,
        name: String,
        deleteBranch: Boolean,
        tagMessage: String,
    ) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            val config = readConfig(git.repository.config)
            val branch = prefix(config, type) + name
            val branchId = git.repository.resolve(branch) ?: throw IllegalArgumentException("Branch $branch was not found")
            if (type == GitFlowBranchType.Feature) {
                checkoutAndMerge(git, config.develop, branch, branchId.name)
            } else {
                checkoutAndMerge(git, config.master, branch, branchId.name)
                if (tagMessage.isNotBlank() || type == GitFlowBranchType.Release || type == GitFlowBranchType.Hotfix) {
                    git.tag()
                        .setName(config.versionTagPrefix + name)
                        .setMessage(tagMessage.ifBlank { name })
                        .call()
                }
                checkoutAndMerge(git, config.develop, config.master, git.repository.resolve(config.master)?.name)
            }
            if (deleteBranch) {
                git.branchDelete().setBranchNames(branch).setForce(false).call()
            }
            git.checkout().setName(if (type == GitFlowBranchType.Feature) config.develop else config.develop).call()
        }
    }

    private fun checkoutAndMerge(git: org.eclipse.jgit.api.Git, into: String, fromName: String, fromId: String?) {
        git.checkout().setName(into).call()
        val id = fromId ?: return
        git.merge()
            .include(git.repository.resolve(id))
            .setFastForward(MergeCommand.FastForwardMode.NO_FF)
            .setMessage("Merge branch '$fromName' into $into")
            .call()
    }

    private fun writeConfig(stored: org.eclipse.jgit.lib.StoredConfig, config: GitFlowConfig) {
        stored.setString("gitflow", "branch", "master", config.master)
        stored.setString("gitflow", "branch", "develop", config.develop)
        stored.setString("gitflow", "prefix", "feature", config.featurePrefix)
        stored.setString("gitflow", "prefix", "release", config.releasePrefix)
        stored.setString("gitflow", "prefix", "hotfix", config.hotfixPrefix)
        stored.setString("gitflow", "prefix", "support", config.supportPrefix)
        stored.setString("gitflow", "prefix", "versiontag", config.versionTagPrefix)
        stored.save()
    }

    private fun readConfig(config: org.eclipse.jgit.lib.StoredConfig) = GitFlowConfig(
        master = config.getString("gitflow", "branch", "master") ?: "main",
        develop = config.getString("gitflow", "branch", "develop") ?: "develop",
        featurePrefix = config.getString("gitflow", "prefix", "feature") ?: "feature/",
        releasePrefix = config.getString("gitflow", "prefix", "release") ?: "release/",
        hotfixPrefix = config.getString("gitflow", "prefix", "hotfix") ?: "hotfix/",
        supportPrefix = config.getString("gitflow", "prefix", "support") ?: "support/",
        versionTagPrefix = config.getString("gitflow", "prefix", "versiontag") ?: "",
    )

    private fun prefix(config: GitFlowConfig, type: GitFlowBranchType) = when (type) {
        GitFlowBranchType.Feature -> config.featurePrefix
        GitFlowBranchType.Release -> config.releasePrefix
        GitFlowBranchType.Hotfix -> config.hotfixPrefix
        GitFlowBranchType.Support -> config.supportPrefix
    }
}
