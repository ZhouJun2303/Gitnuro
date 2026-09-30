package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.extensions.countOrZero
import com.zhoujun.awegit.domain.models.Status
import com.zhoujun.awegit.domain.models.StatusSummary
import com.zhoujun.awegit.domain.models.StatusType
import javax.inject.Inject

class GetSummaryFromStatusUseCase @Inject constructor() {
    operator fun invoke(status: Status): StatusSummary {
        val staged = status.staged

        val unstaged = status.unstaged
        val allChanges = staged + unstaged

        val groupedChanges = allChanges.groupBy {
            it.statusType
        }

        val deletedCount = groupedChanges[StatusType.REMOVED].countOrZero()
        val addedCount = groupedChanges[StatusType.ADDED].countOrZero()

        val modifiedCount = groupedChanges[StatusType.MODIFIED].countOrZero()
        val conflictingCount = groupedChanges[StatusType.CONFLICTING].countOrZero()

        return StatusSummary(
            modifiedCount = modifiedCount,
            deletedCount = deletedCount,
            addedCount = addedCount,
            conflictingCount = conflictingCount,
        )
    }
}