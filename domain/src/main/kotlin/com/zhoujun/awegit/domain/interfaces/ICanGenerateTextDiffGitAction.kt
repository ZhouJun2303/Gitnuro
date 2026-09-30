package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.models.EntryContent
import org.eclipse.jgit.diff.RawText

interface ICanGenerateTextDiffGitAction {
    suspend operator fun invoke(
        rawOld: EntryContent,
        rawNew: EntryContent,
        onText: suspend (oldRawText: RawText, newRawText: RawText) -> Unit,
    ): Boolean
}