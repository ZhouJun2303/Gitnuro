package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import org.eclipse.jgit.api.Git

interface IDeleteLocallyRemoteBranchesGitAction {
    suspend operator fun invoke(repositoryPath: String, branches: List<String>): Either<List<String>, GitError>
}