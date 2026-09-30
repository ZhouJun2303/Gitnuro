package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Commit
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.Ref
import org.eclipse.jgit.revwalk.RevCommit

interface ICreateBranchGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        branchName: String,
        targetCommit: Commit?,
        checkout: Boolean = true,
    ): Either<Unit, GitError>
}