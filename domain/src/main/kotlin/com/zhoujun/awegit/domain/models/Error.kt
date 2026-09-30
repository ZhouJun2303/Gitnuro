package com.zhoujun.awegit.domain.models

import com.zhoujun.awegit.domain.exceptions.AweGitException

data class Error(
    val taskType: TaskType,
    val date: Long,
    val exception: Exception,
    val isUnhandled: Boolean,
)


fun newErrorNow(
    taskType: TaskType,
    exception: Exception,
): Error {
    return Error(
        taskType = taskType,
        date = System.currentTimeMillis(),
        exception = exception,
        isUnhandled = exception !is AweGitException
    )
}
