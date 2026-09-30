package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.usecases.CreateTagUseCase
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject

class CreateTagViewModel @AssistedInject constructor(
    private val createTagUseCase: CreateTagUseCase,
    @Assisted val targetCommit: Commit,
) : TabViewModel() {
    @AssistedFactory
    interface Factory {
        fun create(commit: Commit): CreateTagViewModel
    }

    fun createTag(name: String, message: String, pushToOrigin: Boolean) {
        createTagUseCase(name, targetCommit, message, pushToOrigin)
    }
}
