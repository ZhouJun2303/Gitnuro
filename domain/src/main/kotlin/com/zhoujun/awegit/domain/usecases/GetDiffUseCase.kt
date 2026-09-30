package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.common.extensions.TAG
import com.zhoujun.awegit.common.printError
import com.zhoujun.awegit.domain.errors.okOrNull
import com.zhoujun.awegit.domain.interfaces.IFormatDiffGitAction
import com.zhoujun.awegit.domain.interfaces.IGenerateSplitHunkFromDiffResultGitAction
import com.zhoujun.awegit.domain.models.DiffResult
import com.zhoujun.awegit.domain.models.DiffTextViewType
import com.zhoujun.awegit.domain.models.DiffType
import com.zhoujun.awegit.domain.models.ViewDiffResult
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.services.AppSettingsService
import org.eclipse.jgit.diff.DiffEntry
import javax.inject.Inject

class GetDiffUseCase @Inject constructor(
    private val formatDiffGitAction: IFormatDiffGitAction,
    private val generateSplitHunkFromDiffResultGitAction: IGenerateSplitHunkFromDiffResultGitAction,
    private val repositoryDataRepository: RepositoryDataRepository,
) {
    suspend operator fun invoke(diffType: DiffType, diffViewType: DiffTextViewType, isDisplayFullFile: Boolean): ViewDiffResult {
        val repositoryPath = repositoryDataRepository.repositoryPath ?: return ViewDiffResult.None

        return try {
            val diffFormat = formatDiffGitAction(repositoryPath, diffType, isDisplayFullFile).okOrNull()!!
            val diffEntry = diffFormat.diffEntry
            if (
                diffViewType == DiffTextViewType.Split &&
                diffFormat is DiffResult.Text &&
                diffEntry.changeType != DiffEntry.ChangeType.ADD &&
                diffEntry.changeType != DiffEntry.ChangeType.DELETE
            ) {
                val splitHunkList = generateSplitHunkFromDiffResultGitAction(diffFormat)
                ViewDiffResult.Loaded(
                    diffType,
                    DiffResult.TextSplit(diffEntry, splitHunkList)
                )
            } else {
                ViewDiffResult.Loaded(diffType, diffFormat)
            }

        } catch (ex: Exception) {
            printError(TAG, ex.message.orEmpty(), ex)

            ex.printStackTrace()
            ViewDiffResult.DiffNotFound(diffType)
        }
    }
}