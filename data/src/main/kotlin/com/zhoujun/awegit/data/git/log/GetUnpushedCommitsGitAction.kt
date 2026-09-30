package com.zhoujun.awegit.data.git.log

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IGetUnpushedCommitsGitAction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import javax.inject.Inject

class GetUnpushedCommitsGitAction @Inject constructor(
    private val jgit: JGit,
) : IGetUnpushedCommitsGitAction {
    override suspend fun invoke(repositoryPath: String) = withContext(Dispatchers.IO) {
        jgit.provide(repositoryPath) { git ->
            UnpushedCommitsCalculator.compute(git.repository)
        }
    }
}
