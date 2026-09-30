package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.RebaseInteractiveState
import org.eclipse.jgit.api.Git

interface IGetRebaseInteractiveStateGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<RebaseInteractiveState, GitError>
}