package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.LfsError
import com.zhoujun.awegit.domain.lfs.LfsObjectBatch
import com.zhoujun.awegit.domain.lfs.LfsObjects
import com.zhoujun.awegit.domain.models.OperationType

interface IGetLfsObjectsGitAction {
    suspend operator fun invoke(
        lfsServerUrl: String,
        operationType: OperationType,
        branch: String,
        lfsObjectBatches: List<LfsObjectBatch>,
        headers: Map<String, String>,
    ): Either<LfsObjects, LfsError>
}