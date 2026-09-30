package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.AuthorInfo
import org.eclipse.jgit.api.Git

interface ILoadAuthorGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<AuthorInfo, GitError>
}