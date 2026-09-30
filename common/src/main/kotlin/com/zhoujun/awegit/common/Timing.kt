package com.zhoujun.awegit.common

inline fun <T> measureAndLog(tag: String, label: String, block: () -> T): T {
    val start = System.nanoTime()
    return block().also { printLog(tag, "$label took ${(System.nanoTime() - start) / 1_000_000} ms") }
}
