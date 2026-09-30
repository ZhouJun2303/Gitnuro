package com.zhoujun.awegit.domain.usecases

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import javax.inject.Inject

class ScanForRepositoriesUseCase @Inject constructor() {
    private val skip = setOf("node_modules", ".gradle", "build", "target", ".idea", "vendor", "dist", "out")

    suspend operator fun invoke(root: String, maxDepth: Int = 4): List<String> = withContext(Dispatchers.IO) {
        val found = mutableListOf<String>()
        fun visit(dir: File, depth: Int) {
            if (File(dir, ".git").exists()) {
                found += dir.absolutePath
                return
            }
            if (depth >= maxDepth) return
            dir.listFiles { file -> file.isDirectory && !file.isHidden && file.name !in skip }?.forEach { visit(it, depth + 1) }
        }
        visit(File(root), 0)
        found.sorted()
    }
}
