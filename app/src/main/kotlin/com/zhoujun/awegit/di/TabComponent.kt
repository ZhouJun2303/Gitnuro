package com.zhoujun.awegit.di

import com.zhoujun.awegit.common.TabScope
import com.zhoujun.awegit.di.modules.FileWatcherModule
import com.zhoujun.awegit.di.modules.TabModule
import com.zhoujun.awegit.di.modules.TabRepositoriesModule
import com.zhoujun.awegit.di.modules.TabScopeGitActionsModule
import com.zhoujun.awegit.repositoryopen.RepositoryOpenViewModel
import com.zhoujun.awegit.ui.dialogs.*
import com.zhoujun.awegit.viewmodels.*
import com.zhoujun.awegit.viewmodels.sidepanel.SubmoduleDialogViewModel
import dagger.Subcomponent

@TabScope
@Subcomponent(
    modules = [
        TabModule::class,
        TabRepositoriesModule::class,
        FileWatcherModule::class,
        TabScopeGitActionsModule::class,
    ],
)
interface TabComponent {
    @Subcomponent.Factory
    interface Factory {
        fun create(): TabComponent
    }

    fun cloneViewModel(): CloneViewModel
    fun settingsViewModel(): SettingsViewModel
    fun repositoryTabViewModelFactory(): RepositoryTabViewModel.Factory
    fun repositoryOpenViewModel(): RepositoryOpenViewModel
    fun historyViewModel(): HistoryViewModel
    fun authorViewModel(): AuthorViewModel
    fun stashWithMessageViewModel(): StashWithMessageViewModel
    fun quickActionsViewModel(): QuickActionsViewModel
    fun quickLaunchViewModel(): QuickLaunchViewModel
    fun repositorySettingsViewModel(): RepositorySettingsViewModel
    fun gitFlowInitViewModel(): GitFlowInitViewModel
    fun gitFlowStartViewModel(): GitFlowStartViewModel
    fun gitFlowFinishViewModel(): GitFlowFinishViewModel
    fun resolveConflictViewModelFactory(): ResolveConflictViewModel.Factory
    fun setUpstreamBranchDialogViewModelFactory(): SetUpstreamBranchDialogViewModel.Factory
    fun renameBranchDialogViewModelFactory(): RenameBranchDialogViewModel.Factory
    fun createBranchViewModelFactory(): CreateBranchViewModel.Factory
    fun createTagViewModelFactory(): CreateTagViewModel.Factory
    fun resetBranchViewModelFactory(): ResetBranchViewModel.Factory
    fun addEditRemoteViewModelFactory(): AddEditRemoteViewModel.Factory
    fun pullDialogViewModelFactory(): PullDialogViewModel.Factory
    fun mergeDialogViewModelFactory(): MergeDialogViewModel.Factory
    fun rebaseDialogViewModelFactory(): RebaseDialogViewModel.Factory
    fun checkoutRemoteBranchDialogViewModelFactory(): CheckoutRemoteBranchDialogViewModel.Factory
    fun deleteConfirmDialogViewModel(): DeleteConfirmDialogViewModel
    fun fetchDialogViewModel(): FetchDialogViewModel
    fun pushDialogViewModel(): PushDialogViewModel
    fun rewordCommitDialogViewModelFactory(): RewordCommitDialogViewModel.Factory
    fun submoduleDialogViewModel(): SubmoduleDialogViewModel
    fun signOffDialogViewModel(): SignOffDialogViewModel
}