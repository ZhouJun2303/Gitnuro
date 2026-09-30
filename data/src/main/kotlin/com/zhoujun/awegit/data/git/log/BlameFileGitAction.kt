package com.zhoujun.awegit.data.git.log

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IBlameFileGitAction
import javax.inject.Inject

class BlameFileGitAction @Inject constructor(
    private val jgit: JGit,
) : IBlameFileGitAction {
    override suspend fun invoke(
        repositoryPath: String,
        filePath: String
    ) = jgit.provide(repositoryPath) { git ->
        git.blame()
            .setFilePath(filePath)
            .setFollowFileRenames(true)
            .call() ?: throw Exception("File is no longer present in the workspace and can't be blamed")
    }
}