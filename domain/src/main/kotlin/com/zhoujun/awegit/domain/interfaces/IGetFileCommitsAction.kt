package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Commit
import org.eclipse.jgit.api.Git

interface IGetFileCommitsAction {
    suspend operator fun invoke(
        repositoryPath: String,
        filePath: String
    ): Either<List<Commit>, GitError>
}