package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IDeleteLocallyRemoteBranchesGitAction
import com.zhoujun.awegit.domain.interfaces.IDeleteRemoteGitAction
import com.zhoujun.awegit.domain.models.RemoteInfo
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class DeleteRemoteInfoUseCase @Inject constructor(
    private val deleteRemoteGitAction: IDeleteRemoteGitAction,
    private val deleteLocallyRemoteBranchesGitAction: IDeleteLocallyRemoteBranchesGitAction,
    private val useCaseExecutor: UseCaseExecutor,

    ) {
    operator fun invoke(remoteInfo: RemoteInfo) {
        useCaseExecutor.executeLaunch(
            taskType = TaskType.DeleteRemote,
            dataToRefresh = arrayOf(DataToRefresh.ALL),
        ) { repositoryPath ->
            deleteRemoteGitAction(repositoryPath, remoteInfo.remote.name)

            val remoteBranchesToDelete = remoteInfo.branchesList

            deleteLocallyRemoteBranchesGitAction(repositoryPath, remoteBranchesToDelete.map { it.name })
        }
    }
}
