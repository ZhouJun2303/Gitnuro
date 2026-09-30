package com.zhoujun.awegit.domain.commands

import com.zhoujun.awegit.common.OS
import com.zhoujun.awegit.common.currentOs

data class PreparedCommand(
    val program: List<String>,
    val environment: Map<String, String>,
)

object CustomCommandPreparer {
    fun prepare(
        template: String,
        repo: String,
        sha: String,
        branch: String,
        file: String,
    ): PreparedCommand {
        val values = mapOf("repo" to repo, "sha" to sha, "branch" to branch, "file" to file)
        if (currentOs == OS.WINDOWS && values.values.any { '"' in it || '%' in it }) {
            throw IllegalArgumentException("Refusing to run a command because a value contains \" or %")
        }
        val expanded = template
            .replace("\${repo}", if (currentOs == OS.WINDOWS) "%AWEGIT_REPO%" else "\"\$AWEGIT_REPO\"")
            .replace("\${sha}", if (currentOs == OS.WINDOWS) "%AWEGIT_SHA%" else "\"\$AWEGIT_SHA\"")
            .replace("\${branch}", if (currentOs == OS.WINDOWS) "%AWEGIT_BRANCH%" else "\"\$AWEGIT_BRANCH\"")
            .replace("\${file}", if (currentOs == OS.WINDOWS) "%AWEGIT_FILE%" else "\"\$AWEGIT_FILE\"")
        val program = if (currentOs == OS.WINDOWS) {
            listOf("cmd", "/c", expanded)
        } else {
            listOf("/bin/sh", "-c", expanded)
        }
        return PreparedCommand(
            program = program,
            environment = mapOf(
                "AWEGIT_REPO" to repo,
                "AWEGIT_SHA" to sha,
                "AWEGIT_BRANCH" to branch,
                "AWEGIT_FILE" to file,
            ),
        )
    }
}
