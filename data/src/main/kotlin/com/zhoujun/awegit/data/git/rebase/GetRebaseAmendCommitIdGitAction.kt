package com.zhoujun.awegit.data.git.rebase

import com.zhoujun.awegit.domain.RebaseConstants
import com.zhoujun.awegit.domain.interfaces.IGetRebaseAmendCommitIdGitAction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.api.RebaseCommand
import java.io.File
import javax.inject.Inject

class GetRebaseAmendCommitIdGitAction @Inject constructor() : IGetRebaseAmendCommitIdGitAction {
    override suspend operator fun invoke(git: Git): String? = withContext(Dispatchers.IO) {
        val repository = git.repository

        val amendFile = File(repository.directory, "${RebaseCommand.REBASE_MERGE}/${RebaseConstants.AMEND}")

        if (!amendFile.exists()) {
            return@withContext null
        }

        return@withContext amendFile.readText().removeSuffix("\n").removeSuffix("\r\n")
    }
}