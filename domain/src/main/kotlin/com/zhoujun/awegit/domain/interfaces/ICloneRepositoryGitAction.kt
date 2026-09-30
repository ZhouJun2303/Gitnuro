package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.models.CloneState
import kotlinx.coroutines.flow.Flow
import java.io.File

interface ICloneRepositoryGitAction {
    operator fun invoke(directory: File, url: String, cloneSubmodules: Boolean): Flow<CloneState>
}