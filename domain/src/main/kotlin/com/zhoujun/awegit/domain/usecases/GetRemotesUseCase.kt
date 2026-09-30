package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.errors.either
import com.zhoujun.awegit.domain.interfaces.IGetRemoteBranchesGitAction
import com.zhoujun.awegit.domain.interfaces.IGetRemotesGitAction
import com.zhoujun.awegit.domain.models.RemoteInfo
import javax.inject.Inject

class GetRemotesUseCase @Inject constructor(
    private val getRemotesGitAction: IGetRemotesGitAction,
    private val getRemoteBranchesGitAction: IGetRemoteBranchesGitAction,
    private val useCaseExecutor: UseCaseExecutor,
) {
    suspend operator fun invoke() = either {
        useCaseExecutor.execute { repositoryPath ->
            getRemoteInfoList(repositoryPath)
        }
    }
    private suspend fun getRemoteInfoList(repositoryPath: String) = either<List<RemoteInfo>, GitError> {
        val allRemoteBranches = getRemoteBranchesGitAction(repositoryPath).bind()
        val remoteInfoList = getRemotesGitAction(repositoryPath).bind()

        val remotes = remoteInfoList.map { remote ->
            val remoteBranches = allRemoteBranches.filter { branch ->
                branch.name.startsWith("refs/remotes/${remote.name}")
            }
            RemoteInfo(remote, remoteBranches)
        }

        Either.Ok(remotes)
    }
}