package com.zhoujun.awegit.data.git.log

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IGetRecentCommitMessagesGitAction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.errors.NoHeadException
import javax.inject.Inject

class GetRecentCommitMessagesGitAction @Inject constructor(
    private val jgit: JGit,
) : IGetRecentCommitMessagesGitAction {
    override suspend fun invoke(repositoryPath: String, count: Int) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            try {
                git.log().setMaxCount(count).call().map { it.shortMessage }
            } catch (_: NoHeadException) {
                emptyList()
            }
        }
    }
}
