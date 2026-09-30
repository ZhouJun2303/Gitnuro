package com.zhoujun.awegit.data.git

import com.zhoujun.awegit.domain.interfaces.IGetWorktreePathGitAction
import org.eclipse.jgit.api.Git
import javax.inject.Inject

class GetWorktreePathGitAction @Inject constructor(
    private val jgit: JGit,
) : IGetWorktreePathGitAction {
    override suspend operator fun invoke(repositoryPath: String) = jgit.provide(repositoryPath) { git ->
        git.repository.workTree.absolutePath
    }
}