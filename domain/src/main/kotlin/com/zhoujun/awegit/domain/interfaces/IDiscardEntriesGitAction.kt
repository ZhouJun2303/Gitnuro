package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.StatusEntry

interface IDiscardEntriesGitAction {
    suspend operator fun invoke(repositoryPath: String, statusEntries: List<StatusEntry>, staged: Boolean): Either<Unit, GitError>
}