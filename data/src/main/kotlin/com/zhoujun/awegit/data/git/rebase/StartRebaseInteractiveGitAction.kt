package com.zhoujun.awegit.data.git.rebase

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.exceptions.UncommittedChangesDetectedException
import com.zhoujun.awegit.domain.interfaces.IStartRebaseInteractiveGitAction
import com.zhoujun.awegit.domain.models.Commit
import org.eclipse.jgit.api.RebaseCommand
import org.eclipse.jgit.api.RebaseResult
import org.eclipse.jgit.lib.RebaseTodoLine
import javax.inject.Inject


class StartRebaseInteractiveGitAction @Inject constructor(
    private val jgit: JGit,
) : IStartRebaseInteractiveGitAction {
    override suspend operator fun invoke(
        repositoryPath: String,
        commit: Commit,
    ): Either<Unit, GitError> = jgit.provide(repositoryPath) { git ->
        val base = git
            .repository
            .resolve(commit.hash) ?: throw Exception("Commit ${commit.hash} not found")

        val interactiveHandler = object : RebaseCommand.InteractiveHandler {
            override fun prepareSteps(steps: MutableList<RebaseTodoLine>?) {}
            override fun modifyCommitMessage(message: String?): String = ""
        }

        val rebaseResult = git
            .rebase()
            .runInteractively(interactiveHandler, true)
            .setOperation(RebaseCommand.Operation.BEGIN)
            .setUpstream(base)
            .call()

        when (rebaseResult.status) {
            RebaseResult.Status.FAILED -> throw UncommittedChangesDetectedException("Rebase interactive failed.")
            RebaseResult.Status.UNCOMMITTED_CHANGES, RebaseResult.Status.CONFLICTS -> throw UncommittedChangesDetectedException(
                "You can't have uncommitted changes before starting a rebase interactive"
            )

            else -> {}
        }
    }
}