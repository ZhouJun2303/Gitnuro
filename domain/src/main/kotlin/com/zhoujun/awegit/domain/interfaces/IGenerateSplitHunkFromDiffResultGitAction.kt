package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.models.DiffResult
import com.zhoujun.awegit.domain.models.SplitHunk

interface IGenerateSplitHunkFromDiffResultGitAction {
    operator fun invoke(diffFormat: DiffResult.Text): List<SplitHunk>
}