package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.usecases.DataToRefresh
import com.zhoujun.awegit.domain.usecases.GetWorktreeUseCase
import com.zhoujun.awegit.domain.usecases.OpenPathInSystemUseCase
import com.zhoujun.awegit.domain.usecases.RefreshDataUseCase
import kotlinx.coroutines.launch
import javax.inject.Inject

class QuickActionsViewModel @Inject constructor(
    private val refreshDataUseCase: RefreshDataUseCase,
    private val getWorktreeUseCase: GetWorktreeUseCase,
    private val openPathInSystemUseCase: OpenPathInSystemUseCase,
) : TabViewModel() {

    // TODO Implement bunch of methods

    fun refreshRepository() = refreshDataUseCase(DataToRefresh.ALL)

    fun openProjectInFileExplorer() {
        viewModelScope.launch {
            val worktree = getWorktreeUseCase()

            if (worktree is Either.Ok) {
                openPathInSystemUseCase(worktree.value)
            }
        }
    }
}