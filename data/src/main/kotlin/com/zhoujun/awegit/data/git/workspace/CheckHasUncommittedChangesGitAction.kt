package com.zhoujun.awegit.data.git.workspace

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.extensions.hasUntrackedChanges
import com.zhoujun.awegit.domain.interfaces.ICheckHasUncommittedChangesGitAction
import javax.inject.Inject

class CheckHasUncommittedChangesGitAction @Inject constructor(
    private val jgit: JGit,
) : ICheckHasUncommittedChangesGitAction {
    override suspend operator fun invoke(repositoryPath: String): Either<Boolean, GitError> = jgit.provide(repositoryPath) { git ->
        val status = git
            .status()
            .call()

        status.hasUncommittedChanges() || status.hasUntrackedChanges()
    }
}