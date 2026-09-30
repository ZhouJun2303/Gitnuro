package com.zhoujun.awegit.data.git.workspace

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GenericError
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.errors.raiseError
import com.zhoujun.awegit.domain.interfaces.IDeleteFileGitAction
import com.zhoujun.awegit.domain.interfaces.IDiscardEntriesGitAction
import com.zhoujun.awegit.domain.models.StatusEntry
import com.zhoujun.awegit.domain.models.StatusType
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.Git
import java.io.File
import javax.inject.Inject

class DeleteFileGitAction @Inject constructor(
    private val jgit: JGit,
) : IDeleteFileGitAction {
    override suspend fun invoke(
        repositoryPath: String,
        filePath: String
    ) = jgit.provide(repositoryPath) { git ->
        val fileToDelete = File(git.repository.workTree, filePath)

        if (!fileToDelete.deleteRecursively()) {
            raiseError(GenericError("Delete file recursively failed"))
        }
    }
}