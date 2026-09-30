package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.Ref

interface IUnstageByDirectoryGitAction {
    suspend operator fun invoke(repositoryPath: String, dir: String): Either<Unit, GitError>
}