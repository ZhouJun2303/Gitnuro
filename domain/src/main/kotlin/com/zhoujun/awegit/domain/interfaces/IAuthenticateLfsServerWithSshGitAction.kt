package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.lfs.LfsSshAuthenticateResult
import com.zhoujun.awegit.domain.models.OperationType

interface IAuthenticateLfsServerWithSshGitAction {
    suspend operator fun invoke(
        lfsServerUrl: String,
        operationType: OperationType,
    ): LfsSshAuthenticateResult
}