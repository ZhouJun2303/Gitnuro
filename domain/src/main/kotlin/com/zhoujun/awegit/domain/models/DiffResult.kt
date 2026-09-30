package com.zhoujun.awegit.domain.models

import org.eclipse.jgit.diff.DiffEntry

sealed class DiffResult(
    val diffEntry: DiffEntry,
) {
    sealed class TextDiff(diffEntry: DiffEntry) : DiffResult(diffEntry)

    class Text(
        diffEntry: DiffEntry,
        val hunks: List<Hunk>,
    ) : TextDiff(diffEntry)

    class TextSplit(
        diffEntry: DiffEntry,
        val hunks: List<SplitHunk>,
    ) : TextDiff(diffEntry)

    class NonText(
        diffEntry: DiffEntry,
        val oldBinaryContent: EntryContent,
        val newBinaryContent: EntryContent,
    ) : DiffResult(diffEntry)

    class Submodule(
        diffEntry: DiffEntry,
        val submodule: com.zhoujun.awegit.domain.models.Submodule?,
    ) : DiffResult(diffEntry)
}