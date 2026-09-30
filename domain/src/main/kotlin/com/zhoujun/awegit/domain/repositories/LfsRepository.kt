package com.zhoujun.awegit.domain.repositories

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.LfsError
import com.zhoujun.awegit.domain.lfs.LfsObjectBatch
import com.zhoujun.awegit.domain.lfs.LfsObjects
import com.zhoujun.awegit.domain.lfs.LfsPrepareUploadObjectBatch
import com.zhoujun.awegit.domain.models.OperationType
import java.nio.file.Path

interface LfsRepository {
    suspend fun postBatchObjects(
        remoteUrl: String,
        lfsPrepareUploadObjectBatch: LfsPrepareUploadObjectBatch,
        headers: Map<String, String>,
        username: String?,
        password: String?,
    ): Either<LfsObjects, LfsError>

    suspend fun uploadObject(
        uploadUrl: String,
        oid: String,
        file: Path,
        size: Long,
        headers: Map<String, String>,
        username: String?,
        password: String?,
    ): Either<Unit, LfsError>

    suspend fun verify(
        url: String,
        oid: String,
        size: Long,
        headers: Map<String, String>,
        username: String?,
        password: String?,
    ): Either<Unit, LfsError>

    suspend fun downloadObject(
        downloadUrl: String,
        outPath: Path,
        headers: Map<String, String>,
        username: String?,
        password: String?,
    ): Either<Unit, LfsError>

    suspend fun getLfsObjects(
        lfsServerUrl: String,
        operationType: OperationType,
        branch: String,
        objects: List<LfsObjectBatch>,
        username: String?,
        password: String?,
        headers: Map<String, String>,
    ): Either<LfsObjects, LfsError>
}