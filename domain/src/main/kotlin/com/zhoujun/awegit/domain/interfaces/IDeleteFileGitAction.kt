package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.StatusEntry
import org.eclipse.jgit.api.Git

interface IDeleteFileGitAction {
    suspend operator fun invoke(repositoryPath: String, filePath: String): Either<Unit, GitError>
}