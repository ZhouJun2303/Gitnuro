package com.zhoujun.awegit.domain.conflicts

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ConflictParserTest {
    @Test
    fun `plain conflict`() {
        val text = """
            before
            <<<<<<< HEAD
            mine
            =======
            theirs
            >>>>>>> branch
            after
        """.trimIndent()
        val blocks = ConflictParser.parse(text)
        assertTrue(blocks[0] is ConflictBlock.Common)
        val conflict = blocks[1] as ConflictBlock.Conflict
        assertEquals(listOf("mine"), conflict.mine)
        assertNull(conflict.base)
        assertEquals(listOf("theirs"), conflict.theirs)
        assertEquals(listOf("after"), (blocks[2] as ConflictBlock.Common).lines)
    }

    @Test
    fun `diff3 conflict`() {
        val text = """
            <<<<<<< HEAD
            mine
            ||||||| base
            original
            =======
            theirs
            >>>>>>> branch
        """.trimIndent()
        val conflict = ConflictParser.parse(text).filterIsInstance<ConflictBlock.Conflict>().single()
        assertEquals(listOf("original"), conflict.base)
        assertEquals(listOf("mine"), conflict.mine)
        assertEquals(listOf("theirs"), conflict.theirs)
    }

    @Test
    fun `multiple conflicts`() {
        val text = """
            <<<<<<< HEAD
            a
            =======
            b
            >>>>>>> x
            mid
            <<<<<<< HEAD
            c
            =======
            d
            >>>>>>> y
        """.trimIndent()
        assertEquals(2, ConflictParser.parse(text).filterIsInstance<ConflictBlock.Conflict>().size)
    }

    @Test
    fun `file without conflicts`() {
        val blocks = ConflictParser.parse("one\ntwo")
        assertEquals(1, blocks.size)
        assertEquals(listOf("one", "two"), (blocks.single() as ConflictBlock.Common).lines)
    }
}
