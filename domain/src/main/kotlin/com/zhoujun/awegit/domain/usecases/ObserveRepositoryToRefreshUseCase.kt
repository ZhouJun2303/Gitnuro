package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.FileType
import com.zhoujun.awegit.common.OS
import com.zhoujun.awegit.common.currentOs
import com.zhoujun.awegit.common.measureAndLog
import com.zhoujun.awegit.common.printDebug
import com.zhoujun.awegit.common.printError
import com.zhoujun.awegit.common.systemSeparator
import com.zhoujun.awegit.domain.RepositoryChangeClassifier
import com.zhoujun.awegit.domain.TabCoroutineScope
import com.zhoujun.awegit.domain.errors.okOrNull
import com.zhoujun.awegit.domain.interfaces.IFileChangesWatcher
import com.zhoujun.awegit.domain.interfaces.IGetStatusGitAction
import com.zhoujun.awegit.domain.models.WatcherEvent
import com.zhoujun.awegit.domain.repositories.DataState
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.RepositoryStateRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import java.io.File
import java.nio.file.Paths
import javax.inject.Inject
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

private const val TAG = "ObserveRepositoryToRefreshUseCase"

private const val REFRESH_TIME_SINCE_LAST_OPERATION = 1_500L

class ObserveRepositoryToRefreshUseCase @Inject constructor(
    private val tabCoroutineScope: TabCoroutineScope,
    private val fileChangesWatcher: IFileChangesWatcher,
    private val repositoryDataRepository: RepositoryDataRepository,
    private val getWorktreeUseCase: GetWorktreeUseCase,
    private val refreshDataUseCase: RefreshDataUseCase,
    private val repositoryStateRepository: RepositoryStateRepository,
    private val getStatusGitAction: IGetStatusGitAction,
) {
    private val pending = mutableSetOf<DataToRefresh>()
    private val flushTrigger = Channel<Unit>(Channel.CONFLATED)

    operator fun invoke() {
        val repositoryPath = repositoryDataRepository.repositoryPath ?: return
        tabCoroutineScope.launch {
            val worktreeDir = getWorktreeUseCase().okOrNull() ?: return@launch
            val recursiveWorktreeWatch = currentOs == OS.WINDOWS || currentOs == OS.MAC

            launch {
                for (signal in flushTrigger) {
                    delay(300)
                    while (repositoryStateRepository.currentTask.value != null) delay(200)
                    val sinceLastOp = System.currentTimeMillis() - repositoryStateRepository.lastOperationTimestamp.first()
                    if (sinceLastOp < REFRESH_TIME_SINCE_LAST_OPERATION) {
                        delay(REFRESH_TIME_SINCE_LAST_OPERATION - sinceLastOp)
                    }
                    val toRefresh = synchronized(pending) { pending.toTypedArray().also { pending.clear() } }
                    if (toRefresh.isNotEmpty()) refreshDataUseCase(*toRefresh)
                }
            }

            launch {
                fileChangesWatcher.observeEvents().collect { event ->
                    when (event) {
                        is WatcherEvent.ChangesDetected -> {
                            val relevant = event.changes.map { it.path }.filter { raw ->
                                if (!recursiveWorktreeWatch) return@filter true
                                val path = Paths.get(raw).normalize()
                                val work = Paths.get(worktreeDir).normalize()
                                if (!path.startsWith(work)) return@filter true
                                val rel = work.relativize(path).toString().replace('\\', '/')
                                !isIgnored(rel, ignoredSet())
                            }
                            if (relevant.isEmpty()) return@collect

                            val classification = RepositoryChangeClassifier.classify(relevant, repositoryPath, worktreeDir)
                            if (classification.dataToRefresh.isEmpty()) return@collect

                            printDebug(TAG, "Changes detected: $relevant -> ${classification.dataToRefresh}")
                            enqueue(narrowed(classification.dataToRefresh))

                            if (!recursiveWorktreeWatch) {
                                updateWatchedDirectories(event, repositoryPath, worktreeDir + systemSeparator)
                            }
                        }

                        is WatcherEvent.WatchInitError -> printDebug(TAG, "Watch init error: ${event.code}")
                    }
                }
            }

            measureAndLog(TAG, "watch registration") {
                fileChangesWatcher.addPathToWatch(repositoryPath, false)
                fileChangesWatcher.addPathToWatch("$repositoryPath${systemSeparator}refs", true)
                fileChangesWatcher.addPathToWatch("$repositoryPath${systemSeparator}modules", true)
                if (recursiveWorktreeWatch) {
                    fileChangesWatcher.addPathToWatch(worktreeDir, true)
                } else {
                    fileChangesWatcher.addPathToWatch(worktreeDir, false)
                    val status = repositoryDataRepository.status.first { it !is DataState.Loading }.dataOrNull()
                    val dirs = getDirsToWatch(
                        worktreeDir = worktreeDir + systemSeparator,
                        excludedRelativePaths = HashSet(listOf(".git")),
                        dirFile = File(worktreeDir),
                        ignoreList = status?.ignored.orEmpty(),
                    )
                    if (status != null) {
                        for (child in dirs) {
                            if (!status.ignored.contains(child.absolutePath.removePrefix(repositoryPath))) {
                                fileChangesWatcher.addPathToWatch(child.absolutePath, false)
                            }
                        }
                    }
                }
            }
        }.invokeOnCompletion {
            fileChangesWatcher.close()
        }
    }

    private fun enqueue(types: Set<DataToRefresh>) {
        if (types.isEmpty()) return
        synchronized(pending) { pending += types }
        flushTrigger.trySend(Unit)
    }

    private suspend fun narrowed(types: Set<DataToRefresh>): Set<DataToRefresh> {
        val busy = repositoryStateRepository.currentTask.value != null ||
            System.currentTimeMillis() - repositoryStateRepository.lastOperationTimestamp.first() < REFRESH_TIME_SINCE_LAST_OPERATION
        return if (busy) types.filterTo(mutableSetOf()) { it == DataToRefresh.STATUS } else types
    }

    private fun ignoredSet(): Set<String> =
        repositoryDataRepository.latestStatus?.ignored.orEmpty().map { it.removeSuffix("/") }.toHashSet()

    private fun isIgnored(relPath: String, ignored: Set<String>): Boolean {
        var p = relPath
        while (true) {
            if (p in ignored) return true
            val idx = p.lastIndexOf('/')
            if (idx <= 0) return false
            p = p.substring(0, idx)
        }
    }

    private suspend fun updateWatchedDirectories(
        event: WatcherEvent.ChangesDetected,
        repositoryPath: String,
        worktreeDirPath: String,
    ) {
        val directories = event.changes.filter { it.fileType == FileType.DIRECTORY }
        if (directories.isEmpty()) return

        val groupedDirs = directories.groupBy { File(it.path).exists() }
        val newDirs = groupedDirs[true].orEmpty()
        val removedDirs = groupedDirs[false].orEmpty()

        if (newDirs.isNotEmpty()) {
            val status = getStatusGitAction(
                repositoryPath,
                newDirs.map { it.path.removePrefix(worktreeDirPath) },
            ).okOrNull()

            for (dir in newDirs) {
                if (status != null && !status.ignored.contains(dir.path.removePrefix(worktreeDirPath))) {
                    fileChangesWatcher.addPathToWatch(dir.path, false)
                }
            }

            for (dir in removedDirs) {
                fileChangesWatcher.removePathFromWatch(dir.path)
            }
        }
    }

    private fun getDirsToWatch(
        worktreeDir: String,
        excludedRelativePaths: HashSet<String>,
        dirFile: File,
        ignoreList: List<String>,
    ): List<File> {
        val childrenDirs = try {
            dirFile.listFiles { file -> file.isDirectory }
        } catch (e: Exception) {
            printError(TAG, e.message.orEmpty(), e)
            emptyArray()
        }.filter {
            val relativePath = it.absolutePath.removePrefix(worktreeDir)
            !ignoreList.contains(relativePath) && !excludedRelativePaths.contains(relativePath)
        }

        return childrenDirs + childrenDirs.flatMap {
            getDirsToWatch(worktreeDir, excludedRelativePaths, it, ignoreList)
        }
    }
}
