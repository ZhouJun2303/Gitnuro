package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import org.eclipse.jgit.api.RebaseCommand

interface IResumeRebaseInteractiveGitAction {
    suspend operator fun invoke(repositoryPath: String, interactiveHandler: RebaseCommand.InteractiveHandler): Either<Unit, GitError>
}