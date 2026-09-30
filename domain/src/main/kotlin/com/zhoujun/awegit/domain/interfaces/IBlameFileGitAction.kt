package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import org.eclipse.jgit.blame.BlameResult

interface IBlameFileGitAction {
    suspend operator fun invoke(repositoryPath: String, filePath: String): Either<BlameResult, GitError>
}