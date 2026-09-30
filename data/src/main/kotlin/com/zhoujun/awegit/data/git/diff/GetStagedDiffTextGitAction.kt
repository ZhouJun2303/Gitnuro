package com.zhoujun.awegit.data.git.diff

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IGetStagedDiffTextGitAction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.treewalk.EmptyTreeIterator
import javax.inject.Inject

class GetStagedDiffTextGitAction @Inject constructor(
    private val jgit: JGit,
) : IGetStagedDiffTextGitAction {
    override suspend fun invoke(repositoryPath: String, maxChars: Int) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            val repo = git.repository
            val headTree = repo.resolve("HEAD^{tree}")
            val entries = git.diff().setCached(true).setShowNameAndStatusOnly(true)
                .apply { if (headTree == null) setOldTree(EmptyTreeIterator()) }
                .call()
            DiffTextBuilder.build(repo, entries, maxChars)
        }
    }
}
