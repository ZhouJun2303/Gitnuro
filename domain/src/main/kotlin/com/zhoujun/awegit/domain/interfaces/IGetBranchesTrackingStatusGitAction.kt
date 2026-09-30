package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.TrackingCounts

interface IGetBranchesTrackingStatusGitAction {
    suspend operator fun invoke(repositoryPath: String): Either<Map<String, TrackingCounts>, GitError>
}
