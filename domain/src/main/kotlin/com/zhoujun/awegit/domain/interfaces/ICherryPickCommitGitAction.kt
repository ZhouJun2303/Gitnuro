package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Commit
import org.eclipse.jgit.api.CherryPickResult

interface ICherryPickCommitGitAction {
    suspend operator fun invoke(repositoryPath: String, commit: Commit): Either<Unit, GitError>
}