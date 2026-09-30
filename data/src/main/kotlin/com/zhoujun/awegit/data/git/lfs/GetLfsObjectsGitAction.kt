package com.zhoujun.awegit.data.git.lfs

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.LfsError
import com.zhoujun.awegit.domain.interfaces.IGetLfsObjectsGitAction
import com.zhoujun.awegit.domain.lfs.LfsObjectBatch
import com.zhoujun.awegit.domain.lfs.LfsObjects
import com.zhoujun.awegit.domain.models.OperationType
import com.zhoujun.awegit.domain.network.NetworkConstants
import com.zhoujun.awegit.domain.repositories.LfsRepository
import javax.inject.Inject

class GetLfsObjectsGitAction @Inject constructor(
    private val lfsRepository: LfsRepository,
    private val provideLfsCredentialsGitAction: ProvideLfsCredentialsGitAction,
) : IGetLfsObjectsGitAction {
    override suspend operator fun invoke(
        lfsServerUrl: String,
        operationType: OperationType,
        branch: String,
        lfsObjectBatches: List<LfsObjectBatch>,
        headers: Map<String, String>,
    ): Either<LfsObjects, LfsError> {
        return if (headers.containsKey(NetworkConstants.AUTH_HEADER)) {
            lfsRepository.getLfsObjects(
                lfsServerUrl,
                operationType = operationType,
                branch = branch,
                objects = lfsObjectBatches,
                headers = headers,
                username = null,
                password = null,
            )
        } else {
            provideLfsCredentialsGitAction(
                url = lfsServerUrl,
            ) { user, password ->
                lfsRepository.getLfsObjects(
                    lfsServerUrl,
                    operationType = operationType,
                    branch = branch,
                    objects = lfsObjectBatches,
                    headers = headers,
                    username = user,
                    password = password,
                )
            }
        }
    }
}