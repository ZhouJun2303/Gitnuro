package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.Identity
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.PersonIdent
import org.eclipse.jgit.revwalk.RevCommit

interface IDoCommitGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        message: String,
        amend: Boolean,
        author: Identity?,
    ): Either<Commit, GitError>
}