package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.models.Hunk
import org.eclipse.jgit.diff.RawText
import org.eclipse.jgit.patch.FileHeader

interface IFormatHunksGitAction {
    operator fun invoke(
        fileHeader: FileHeader,
        rawOld: RawText,
        rawNew: RawText,
        isDisplayFullFile: Boolean,
    ): List<Hunk>
}