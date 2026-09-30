package com.zhoujun.awegit.data.git.cli

import java.io.File
import java.util.concurrent.TimeUnit

data class GitCliResult(
    val exitCode: Int,
    val stdout: String,
    val stderr: String,
)

class GitCliRunner @javax.inject.Inject constructor() {
    @Volatile
    private var available: Boolean? = null

    fun isAvailable(): Boolean {
        available?.let { return it }
        val result = run(File(System.getProperty("user.dir")), "--version", timeoutSeconds = 10)
        val ok = result.exitCode == 0
        available = ok
        return ok
    }

    fun run(workDir: File, vararg args: String, timeoutSeconds: Long = 60): GitCliResult {
        val process = ProcessBuilder(listOf("git") + args.toList())
            .directory(workDir)
            .redirectErrorStream(false)
            .start()
        val stdout = StringBuilder()
        val stderr = StringBuilder()
        val outThread = Thread { stdout.append(process.inputStream.bufferedReader().readText()) }
        val errThread = Thread { stderr.append(process.errorStream.bufferedReader().readText()) }
        outThread.start()
        errThread.start()
        val finished = process.waitFor(timeoutSeconds, TimeUnit.SECONDS)
        if (!finished) process.destroyForcibly()
        outThread.join(2_000)
        errThread.join(2_000)
        val code = if (finished) process.exitValue() else -1
        val errText = if (finished) stderr.toString() else stderr.toString() + "\nTimed out"
        return GitCliResult(code, stdout.toString(), errText)
    }
}
