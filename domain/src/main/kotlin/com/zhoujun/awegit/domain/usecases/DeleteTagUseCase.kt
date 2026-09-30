package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.interfaces.IDeleteTagGitAction
import com.zhoujun.awegit.domain.models.Tag
import com.zhoujun.awegit.domain.models.TaskType
import javax.inject.Inject

class DeleteTagUseCase @Inject constructor(
    private val useCaseExecutor: UseCaseExecutor,
    private val deleteTagGitAction: IDeleteTagGitAction,
) {
    operator fun invoke(tag: Tag) = useCaseExecutor.executeLaunch(
        taskType = TaskType.DeleteTag,
        dataToRefresh = arrayOf(DataToRefresh.TAGS, DataToRefresh.LOG),
    ) { repositoryPath ->
        deleteTagGitAction(repositoryPath, tag)
    }
}