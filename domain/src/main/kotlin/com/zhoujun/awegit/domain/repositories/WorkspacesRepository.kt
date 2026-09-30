package com.zhoujun.awegit.domain.repositories

import com.zhoujun.awegit.domain.models.WorkspacesState

interface WorkspacesRepository {
    fun load(): WorkspacesState?
    fun save(state: WorkspacesState)
}
