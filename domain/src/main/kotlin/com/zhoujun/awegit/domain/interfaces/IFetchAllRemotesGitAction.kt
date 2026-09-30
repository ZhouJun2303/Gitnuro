package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Remote

interface IFetchAllRemotesGitAction {
    suspend operator fun invoke(
        repositoryPath: String,
        specificRemote: Remote? = null,
        fetchAllTags: Boolean = false,
        prune: Boolean = true,
    ): Either<Unit, GitError>
}