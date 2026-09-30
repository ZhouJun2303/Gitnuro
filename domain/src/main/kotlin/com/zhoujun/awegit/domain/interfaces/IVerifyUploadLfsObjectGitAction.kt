package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.LfsError
import com.zhoujun.awegit.domain.lfs.LfsObject
import org.eclipse.jgit.lfs.lib.AnyLongObjectId

interface IVerifyUploadLfsObjectGitAction {
    suspend operator fun invoke(
        lfsServerUrl: String,
        lfsObject: LfsObject,
        oid: AnyLongObjectId,
    ): Either<Unit, LfsError>
}