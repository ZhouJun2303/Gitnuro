package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IGetUnpushedCommitsGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<Set<String>, GitError>
}
