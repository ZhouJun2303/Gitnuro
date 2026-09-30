package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Tag
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.Ref

interface IDeleteTagGitAction {
    suspend operator fun invoke(repositoryPath: String, tag: Tag): Either<Unit, GitError>
}