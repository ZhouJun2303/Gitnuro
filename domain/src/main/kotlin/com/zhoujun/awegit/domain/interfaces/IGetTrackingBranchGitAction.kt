package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.TrackingBranch
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.Ref

interface IGetTrackingBranchGitAction {
    suspend operator fun invoke(repositoryPath: String, branch: Branch): Either<TrackingBranch?, GitError>
    suspend operator fun invoke(repositoryPath: String, refName: String): Either<TrackingBranch?, GitError>
}