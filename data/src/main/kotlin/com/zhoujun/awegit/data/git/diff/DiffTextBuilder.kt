package com.zhoujun.awegit.data.git.diff

import com.zhoujun.awegit.domain.models.DiffText
import org.eclipse.jgit.diff.DiffEntry
import org.eclipse.jgit.diff.DiffFormatter
import org.eclipse.jgit.diff.RawTextComparator
import org.eclipse.jgit.lib.Repository
import java.io.ByteArrayOutputStream
import java.io.File
import kotlin.math.max

object DiffTextBuilder {
    private val SKIPPED_FILE_NAMES = setOf(
        "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "Cargo.lock", "Gemfile.lock",
        "poetry.lock", "composer.lock", "go.sum", "gradle.lockfile",
    )

    fun build(repository: Repository, entries: List<DiffEntry>, maxChars: Int): DiffText {
        fun pathOf(e: DiffEntry) = if (e.changeType == DiffEntry.ChangeType.DELETE) e.oldPath else e.newPath
        val files = entries.map { "${it.changeType.name.first()} ${pathOf(it)}" }
        val candidates = entries.filterNot { File(pathOf(it)).name in SKIPPED_FILE_NAMES }
        if (candidates.isEmpty()) return DiffText("", files, false)

        val perFile = max(maxChars / candidates.size, 800)
        val sb = StringBuilder()
        var truncated = false
        for (entry in candidates) {
            val out = ByteArrayOutputStream()
            DiffFormatter(out).use { formatter ->
                formatter.setRepository(repository)
                formatter.setDiffComparator(RawTextComparator.DEFAULT)
                formatter.setContext(3)
                formatter.format(entry)
            }
            var text = out.toString(Charsets.UTF_8)
            if (text.length > perFile) {
                text = text.take(perFile) + "\n... (diff truncated)\n"
                truncated = true
            }
            if (sb.length + text.length > maxChars) {
                sb.append(text.take(max(0, maxChars - sb.length)))
                truncated = true
                break
            }
            sb.append(text)
        }
        return DiffText(sb.toString(), files, truncated)
    }
}
