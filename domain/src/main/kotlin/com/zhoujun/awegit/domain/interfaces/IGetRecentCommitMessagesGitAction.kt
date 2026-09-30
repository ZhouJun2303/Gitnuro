package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError

interface IGetRecentCommitMessagesGitAction {
    suspend operator fun invoke(repositoryPath: String, count: Int): Either<List<String>, GitError>
}
