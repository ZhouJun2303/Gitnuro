package com.zhoujun.awegit.data.git.rebase

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IContinueRebaseGitAction
import org.eclipse.jgit.api.RebaseCommand
import javax.inject.Inject

class ContinueRebaseGitAction @Inject constructor(
    private val jgit: JGit,
) : IContinueRebaseGitAction {
    override suspend operator fun invoke(repositoryPath: String) = jgit.provide(repositoryPath) { git ->
        git.rebase()
            .setOperation(RebaseCommand.Operation.CONTINUE)
            .call()

        // TODO Throw error if call result is not continue?
        Unit
    }
}