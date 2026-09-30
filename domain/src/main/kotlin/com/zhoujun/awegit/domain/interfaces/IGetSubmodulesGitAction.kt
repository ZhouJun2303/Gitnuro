package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Submodule

interface IGetSubmodulesGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<Map<String, Submodule>, GitError>
}