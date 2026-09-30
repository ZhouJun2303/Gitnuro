package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Commit
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.revwalk.RevCommit

interface IStartRebaseInteractiveGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        commit: Commit,
    ): Either<Unit, GitError>
}