package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.Remote
import com.zhoujun.awegit.domain.models.RemoteInfo
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.Ref

interface IGetRemotesGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<List<Remote>, GitError>
}