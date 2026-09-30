package com.zhoujun.awegit.data.git.lfs

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.LfsError
import com.zhoujun.awegit.domain.interfaces.IVerifyUploadLfsObjectGitAction
import com.zhoujun.awegit.domain.lfs.LfsObject
import com.zhoujun.awegit.domain.network.NetworkConstants
import com.zhoujun.awegit.domain.repositories.LfsRepository
import org.eclipse.jgit.lfs.lib.AnyLongObjectId
import javax.inject.Inject


class VerifyUploadLfsObjectGitAction @Inject constructor(
    private val lfsRepository: LfsRepository,
    private val provideLfsCredentialsGitAction: ProvideLfsCredentialsGitAction,
) : IVerifyUploadLfsObjectGitAction {
    override suspend operator fun invoke(
        lfsServerUrl: String,
        lfsObject: LfsObject,
        oid: AnyLongObjectId,
    ): Either<Unit, LfsError> {
        val verifyUrl = lfsObject.actions?.verify?.href

        if (verifyUrl != null) {
            val verifyHeaders = lfsObject.actions?.verify?.header.orEmpty()
            return if (verifyHeaders.containsKey(NetworkConstants.AUTH_HEADER)) {
                lfsRepository.verify(
                    verifyUrl,
                    oid.name(),
                    lfsObject.size,
                    verifyHeaders,
                    null,
                    null,
                )
            } else {
                provideLfsCredentialsGitAction(
                    url = lfsServerUrl,
                ) { user, password ->
                    lfsRepository.verify(
                        verifyUrl,
                        oid.name(),
                        lfsObject.size,
                        verifyHeaders,
                        user,
                        password,
                    )
                }
            }
        }

        return Either.Ok(Unit)
    }
}