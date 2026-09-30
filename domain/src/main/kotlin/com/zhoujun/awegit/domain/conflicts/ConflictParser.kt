package com.zhoujun.awegit.domain.conflicts

sealed interface ConflictBlock {
    data class Common(val lines: List<String>) : ConflictBlock
    data class Conflict(
        val mine: List<String>,
        val base: List<String>?,
        val theirs: List<String>,
        val resolution: List<String>,
    ) : ConflictBlock
}

object ConflictParser {
    fun parse(text: String): List<ConflictBlock> {
        val lines = text.split("\n")
        val blocks = mutableListOf<ConflictBlock>()
        val common = mutableListOf<String>()
        var index = 0
        while (index < lines.size) {
            if (lines[index].startsWith("<<<<<<<")) {
                if (common.isNotEmpty()) {
                    blocks += ConflictBlock.Common(common.toList())
                    common.clear()
                }
                index++
                val mine = mutableListOf<String>()
                val base = mutableListOf<String>()
                val theirs = mutableListOf<String>()
                var sawBase = false
                while (index < lines.size && !lines[index].startsWith("=======")) {
                    if (lines[index].startsWith("|||||||")) {
                        sawBase = true
                        index++
                        while (index < lines.size && !lines[index].startsWith("=======")) {
                            base += lines[index]
                            index++
                        }
                        break
                    }
                    mine += lines[index]
                    index++
                }
                if (index < lines.size && lines[index].startsWith("=======")) index++
                while (index < lines.size && !lines[index].startsWith(">>>>>>>")) {
                    theirs += lines[index]
                    index++
                }
                if (index < lines.size && lines[index].startsWith(">>>>>>>")) index++
                blocks += ConflictBlock.Conflict(
                    mine = mine,
                    base = if (sawBase) base else null,
                    theirs = theirs,
                    resolution = mine,
                )
            } else {
                common += lines[index]
                index++
            }
        }
        if (common.isNotEmpty() || blocks.isEmpty()) blocks += ConflictBlock.Common(common.toList())
        return blocks
    }

    fun render(blocks: List<ConflictBlock>): String {
        return blocks.joinToString("\n") { block ->
            when (block) {
                is ConflictBlock.Common -> block.lines.joinToString("\n")
                is ConflictBlock.Conflict -> block.resolution.joinToString("\n")
            }
        }
    }
}
