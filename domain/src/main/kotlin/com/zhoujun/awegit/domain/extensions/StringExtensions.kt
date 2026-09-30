package com.zhoujun.awegit.domain.extensions

import com.zhoujun.awegit.common.systemSeparator
import com.zhoujun.awegit.domain.exceptions.SshException
import java.security.MessageDigest

private val sha256Cache = java.util.concurrent.ConcurrentHashMap<String, String>()

@OptIn(ExperimentalStdlibApi::class)
val String.sha256: String
    get() = sha256Cache.getOrPut(this) {
        MessageDigest.getInstance("SHA-256")
            .digest(toByteArray(Charsets.UTF_8))
            .toHexString()
    }

val String.dirName: String
    get() {
        val parts = this.split(systemSeparator)

        return if (parts.isNotEmpty())
            parts.last()
        else
            this
    }

val String.dirPath: String
    get() {
        val parts = this.split(systemSeparator).toMutableList()

        return if (parts.count() > 1) {
            parts.removeLast()
            parts.joinToString(systemSeparator)
        } else
            this
    }

fun String.removeLineDelimiters(): String {
    return this
        .removeSuffix("\r\n")
        .removeSuffix("\n")
}

fun String.replaceTabs(): String {
    return this.replace(
        "\t",
        "    "
    )
}

val String.lineDelimiter: String?
    get() {
        return if (this.contains("\r\n"))
            "\r\n"
        else if (this.contains("\n"))
            "\n"
        else
            null
    }

val String.nullIfEmpty: String?
    get() = this.ifBlank { null }

fun String.lowercaseContains(other: String): Boolean {
    return this.lowercase().contains(other.lowercase().trim())
}

fun String.throwIfSshMessage() {
    if (this.isNotEmpty()) {
        throw SshException(this)
    }
}

fun String.removeGitSuffix() = this.removeSuffix(systemSeparator)
    .removeSuffix(".git")
    .removeSuffix(systemSeparator)

fun String.isHttpOrHttps() = this.startsWith("http://") or this.startsWith("https://")