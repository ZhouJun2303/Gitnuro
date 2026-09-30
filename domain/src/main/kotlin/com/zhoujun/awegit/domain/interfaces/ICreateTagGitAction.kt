package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Commit
import org.eclipse.jgit.api.Git

interface ICreateTagGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        tag: String,
        commit: Commit,
        message: String? = null,
        pushToOrigin: Boolean = false,
    ): Either<Unit, GitError>
}