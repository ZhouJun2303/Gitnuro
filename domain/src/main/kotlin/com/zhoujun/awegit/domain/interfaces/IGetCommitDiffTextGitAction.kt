package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.DiffText

interface IGetCommitDiffTextGitAction {
    suspend operator fun invoke(repositoryPath: String, commitHash: String, maxChars: Int): Either<DiffText, GitError>
}
