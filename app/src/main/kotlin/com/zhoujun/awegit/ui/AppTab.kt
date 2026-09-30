package com.zhoujun.awegit.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material.MaterialTheme
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogProperties
import androidx.navigation3.runtime.NavBackStack
import androidx.navigation3.runtime.NavKey
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.scene.DialogSceneStrategy
import androidx.navigation3.ui.NavDisplay
import com.zhoujun.awegit.LoadingRepository
import com.zhoujun.awegit.ProcessingScreen
import com.zhoujun.awegit.Screen
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.lfs
import com.zhoujun.awegit.domain.credentials.CredentialsRequest
import com.zhoujun.awegit.domain.models.NotificationData
import com.zhoujun.awegit.domain.models.NotificationType
import com.zhoujun.awegit.domain.models.PullType
import com.zhoujun.awegit.domain.models.RepositorySelectionState
import com.zhoujun.awegit.domain.models.ui.SelectedItem
import com.zhoujun.awegit.repositoryopen.RepositoryOpenViewModel
import com.zhoujun.awegit.domain.models.successTitle
import com.zhoujun.awegit.domain.repositories.CompletedTask
import com.zhoujun.awegit.repositoryopen.RepositoryOpenPage
import com.zhoujun.awegit.tabViewModel
import com.zhoujun.awegit.theme.dialogOverlay
import com.zhoujun.awegit.ui.components.Notification
import com.zhoujun.awegit.ui.dialogs.*
import com.zhoujun.awegit.ui.dialogs.base.UserPasswordDialog
import com.zhoujun.awegit.ui.dialogs.errors.ErrorDialog
import com.zhoujun.awegit.ui.dialogs.settings.SettingsDialog
import com.zhoujun.awegit.viewmodels.RepositoryTabViewModel
import kotlinx.coroutines.delay
import org.jetbrains.compose.resources.painterResource
import kotlin.time.Duration.Companion.milliseconds


fun <T : NavKey> NavBackStack<T>.addAndRemovePrevious(item: T) {
    this.add(item)

    repeat(lastIndex) {
        this.removeFirst()
    }
}


@Composable
fun AppTab(
    repositoryTabViewModel: RepositoryTabViewModel,
) {
    val errors by repositoryTabViewModel.severeErrors.collectAsState()
    val lastError = errors.firstOrNull()

    val tasks = repositoryTabViewModel.notifications.collectAsState().value
        .toList()
        .sortedBy { it.date }

    val repositorySelectionStatus = repositoryTabViewModel.repositorySelectionState.collectAsState()
    val repositorySelectionStatusValue = repositorySelectionStatus.value
    val processingTask = repositoryTabViewModel.processingTask.collectAsState().value

    val backStack = repositoryTabViewModel.backStack
    val dialogStrategy = remember { DialogSceneStrategy<NavKey>() }


    LaunchedEffect(repositoryTabViewModel) {
        repositoryTabViewModel.loadTab()
    }

    LaunchedEffect(lastError) {
        lastError?.let {
            backStack.add(Screen.Error(it))
        }
    }

    LaunchedEffect(repositorySelectionStatusValue) {
        val screen = when (repositorySelectionStatusValue) {
            RepositorySelectionState.None -> Screen.Welcome

            RepositorySelectionState.Unknown, is RepositorySelectionState.Opening -> Screen.RepositoryLoading

            is RepositorySelectionState.Open -> Screen.RepositoryOpen
        }

        if (!backStack.contains(screen)) {
            backStack.addAndRemovePrevious(screen)
        }
    }

    val dialogsMetadata =
        DialogSceneStrategy.dialog(
            dialogProperties = DialogProperties(
                scrimColor = MaterialTheme.colors.dialogOverlay,
                dismissOnClickOutside = false,
                usePlatformDefaultWidth = false,
            )
        )

    val credentialsState by repositoryTabViewModel.credentialsState.collectAsState()

    LaunchedEffect(credentialsState) {
        val destination = when (val state = credentialsState) {
            is CredentialsRequest.GpgCredentialsRequest -> Screen.GpgCredentials(state)
            CredentialsRequest.HttpCredentialsRequest -> Screen.HttpCredentials
            CredentialsRequest.LfsCredentialsRequest -> Screen.LfsCredentials
            is CredentialsRequest.SshCredentialsRequest -> Screen.SshCredentials(state)
            else -> null
        }

        if (destination != null) {
            backStack.add(destination)
        }
    }

    Box {
        Column(
            modifier = Modifier
                .background(MaterialTheme.colors.surface)
                .fillMaxSize()
        ) {

            Box(modifier = Modifier.fillMaxSize()) {
                NavDisplay(
                    backStack = backStack,
                    onBack = {},
                    sceneStrategies = listOf(dialogStrategy),
                    entryProvider = entryProvider {
                        entry<Screen.Welcome> {
                            WelcomePage(
                                repositoryTabViewModel = repositoryTabViewModel,
                                onShowCloneDialog = { backStack.add(Screen.CloneRepository) },
                                onShowSettings = { backStack.add(Screen.Settings) },
                                onQuickLaunch = { backStack.add(Screen.QuickLaunch) },
                            )
                        }
                        entry<Screen.RepositoryLoading> {
                            val path = (repositorySelectionStatusValue as? RepositorySelectionState.Opening)?.path

                            if (path != null) {
                                LoadingRepository(path)
                            }

                        }
                        entry<Screen.RepositoryOpen> { entry ->
                            val repositoryOpenViewModel = tabViewModel(entry) { it.repositoryOpenViewModel() }

                            RepositoryOpenPage(
                                repositoryOpenViewModel = repositoryOpenViewModel,
                                onNavigate = { backStack.add(it) }
                            )
                        }
                        entry<Screen.Settings>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            val viewModel = tabViewModel(entry, { it.settingsViewModel() })
                            SettingsDialog(
                                settingsViewModel = viewModel,
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.CloneRepository>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            CloneDialog(
                                cloneViewModel = tabViewModel(entry) { it.cloneViewModel() },
                                onClose = { backStack.removeLastOrNull() },
                                onOpenRepository = { dir ->
                                    repositoryTabViewModel.openRepository(dir.absolutePath)
                                },
                            )
                        }
                        entry<Screen.BranchRename>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            RenameBranchDialog(
                                viewModel = tabViewModel(entry) { viewModelsProvider ->
                                    viewModelsProvider
                                        .renameBranchDialogViewModelFactory()
                                        .create(entry.ref)
                                },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.BranchCreate>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            CreateBranchDialog(
                                viewModel = tabViewModel(entry) {
                                    it.createBranchViewModelFactory().create(entry.targetCommit)
                                },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.BranchChangeUpstream>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            SetDefaultUpstreamBranchDialog(
                                viewModel = tabViewModel(entry) { viewModelsProvider ->
                                    viewModelsProvider
                                        .setUpstreamBranchDialogViewModelFactory()
                                        .create(entry.ref)
                                },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.Error>( // TODO Navigating from a dialog (such as add submodule) to this produces a crash
                            metadata = dialogsMetadata
                        ) {
                            ErrorDialog(
                                error = it.error,
                                onAccept = {
                                    backStack.removeLastOrNull()
                                    repositoryTabViewModel.completedTaskAlreadyShown(it.error)
                                },
                            )
                        }
                        entry<Screen.AddEditRemote>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            AddEditRemoteDialog(
                                viewModel = tabViewModel(entry) { viewModelsProvider ->
                                    viewModelsProvider
                                        .addEditRemoteViewModelFactory()
                                        .create(entry.remote)
                                },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.SubmoduleAdd>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            AddSubmodulesDialog(
                                viewModel = tabViewModel(entry) { it.submoduleDialogViewModel() },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.HttpCredentials>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            HttpCredentialsDialog(
                                onDismiss = {
                                    repositoryTabViewModel.credentialsDenied()
                                    backStack.removeLastOrNull()
                                },
                                onAccept = { user, password ->
                                    repositoryTabViewModel.httpCredentialsAccepted(user, password)
                                    backStack.removeLastOrNull()
                                }
                            )
                        }
                        entry<Screen.SshCredentials>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            SshPasswordDialog(
                                credentialsRequest = entry.credentialsRequest,
                                onReject = {
                                    repositoryTabViewModel.credentialsDenied()
                                    backStack.removeLastOrNull()
                                },
                                onAccept = { password ->
                                    repositoryTabViewModel.sshCredentialsAccepted(password)
                                    backStack.removeLastOrNull()
                                }
                            )
                        }
                        entry<Screen.GpgCredentials>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            GpgPasswordDialog(
                                gpgCredentialsRequest = entry.credentialsRequest,
                                onReject = {
                                    repositoryTabViewModel.credentialsDenied()
                                    backStack.removeLastOrNull()
                                },
                                onAccept = { password ->
                                    repositoryTabViewModel.gpgCredentialsAccepted(password)
                                    backStack.removeLastOrNull()
                                }
                            )
                        }
                        entry<Screen.LfsCredentials>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            // TODO Refactor dialogs to have their own view models and not rely on repositoryTabViewModel
                            UserPasswordDialog(
                                title = "LFS Server Credentials",
                                subtitle = "Introduce the credentials for your LFS server",
                                icon = painterResource(Res.drawable.lfs),
                                onDismiss = {
                                    repositoryTabViewModel.credentialsDenied()
                                    backStack.removeLastOrNull()
                                },
                                onAccept = { user, password ->
                                    repositoryTabViewModel.lfsCredentialsAccepted(user, password)
                                    backStack.removeLastOrNull()
                                }
                            )
                        }
                        entry<Screen.SignOffData>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            SignOffDialog(
                                viewModel = tabViewModel(entry) { it.signOffDialogViewModel() },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.TagCreate>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            CreateTagDialog(
                                viewModel = tabViewModel(entry) { viewModelsProvider ->
                                    viewModelsProvider.createTagViewModelFactory().create(entry.targetCommit)
                                },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.BranchReset>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            ResetBranchDialog(
                                viewModel = tabViewModel(entry) { viewModelsProvider ->
                                    viewModelsProvider.resetBranchViewModelFactory().create(entry.targetCommit)
                                },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.QuickActions>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            QuickActionsDialog(
                                viewModel = tabViewModel(entry) { it.quickActionsViewModel() },
                                onDismiss = { backStack.removeLastOrNull() },
                                onShowSignOff = {
                                    backStack.removeLastOrNull()
                                    backStack.add(Screen.SignOffData)
                                },
                                onShowClone = {
                                    backStack.removeLastOrNull()
                                    backStack.add(Screen.Clone)
                                },
                            )
                        }
                        entry<Screen.Author>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            val viewModel = tabViewModel(entry) { it.authorViewModel() }

                            AuthorDialog(
                                viewModel = viewModel,
                                onDismiss = { backStack.removeLastOrNull() }
                            )
                        }
                        entry<Screen.StashWithMessage>(
                            metadata = dialogsMetadata
                        ) { entry ->
                            val viewModel = tabViewModel(entry) { it.stashWithMessageViewModel() }
                            StashWithMessageDialog(
                                viewModel = viewModel,
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.Pull>(metadata = dialogsMetadata) { entry ->
                            PullDialog(
                                viewModel = tabViewModel(entry) { it.pullDialogViewModelFactory().create(entry.remoteBranch) },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.RewordCommit>(metadata = dialogsMetadata) { entry ->
                            RewordCommitDialog(
                                viewModel = tabViewModel(entry) { it.rewordCommitDialogViewModelFactory().create(entry.commit) },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.Fetch>(metadata = dialogsMetadata) {
                            FetchDialog(
                                viewModel = tabViewModel(it) { component -> component.fetchDialogViewModel() },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.Push>(metadata = dialogsMetadata) {
                            PushDialog(
                                viewModel = tabViewModel(it) { component -> component.pushDialogViewModel() },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.Merge>(metadata = dialogsMetadata) { entry ->
                            MergeDialog(
                                viewModel = tabViewModel(entry) { it.mergeDialogViewModelFactory().create(entry.branch) },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.Rebase>(metadata = dialogsMetadata) { entry ->
                            RebaseDialog(
                                viewModel = tabViewModel(entry) { it.rebaseDialogViewModelFactory().create(entry.branch) },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.CheckoutRemoteBranch>(metadata = dialogsMetadata) { entry ->
                            CheckoutRemoteBranchDialog(
                                viewModel = tabViewModel(entry) { it.checkoutRemoteBranchDialogViewModelFactory().create(entry.branch) },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.DeleteBranch>(metadata = dialogsMetadata) { entry ->
                            DeleteBranchDialog(
                                viewModel = tabViewModel(entry) { it.deleteConfirmDialogViewModel() },
                                branch = entry.branch,
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.DeleteTag>(metadata = dialogsMetadata) { entry ->
                            DeleteTagDialog(
                                viewModel = tabViewModel(entry) { it.deleteConfirmDialogViewModel() },
                                tag = entry.tag,
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.DeleteStash>(metadata = dialogsMetadata) { entry ->
                            DeleteStashDialog(
                                viewModel = tabViewModel(entry) { it.deleteConfirmDialogViewModel() },
                                stash = entry.stash,
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.DeleteRemote>(metadata = dialogsMetadata) { entry ->
                            DeleteRemoteDialog(
                                viewModel = tabViewModel(entry) { it.deleteConfirmDialogViewModel() },
                                remoteInfo = entry.remote,
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.QuickLaunch>(metadata = dialogsMetadata) { entry ->
                            QuickLaunchDialog(
                                viewModel = tabViewModel(entry) { it.quickLaunchViewModel() },
                                onDismiss = { backStack.removeLastOrNull() },
                                onCommand = { command ->
                                    handleQuickLaunchCommand(
                                        command = command,
                                        backStack = backStack,
                                        repositoryOpenViewModel = repositoryTabViewModel.viewModelsMap[Screen.RepositoryOpen] as? RepositoryOpenViewModel,
                                    )
                                },
                            )
                        }
                        entry<Screen.RepositorySettings>(metadata = dialogsMetadata) { entry ->
                            RepositorySettingsDialog(
                                viewModel = tabViewModel(entry) { it.repositorySettingsViewModel() },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.GitFlowInit>(metadata = dialogsMetadata) { entry ->
                            GitFlowInitDialog(
                                viewModel = tabViewModel(entry) { it.gitFlowInitViewModel() },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.GitFlowStart>(metadata = dialogsMetadata) { entry ->
                            GitFlowStartDialog(
                                viewModel = tabViewModel(entry) { it.gitFlowStartViewModel() },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.GitFlowFinish>(metadata = dialogsMetadata) { entry ->
                            GitFlowFinishDialog(
                                viewModel = tabViewModel(entry) { it.gitFlowFinishViewModel() },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                        entry<Screen.ResolveConflict>(metadata = dialogsMetadata) { entry ->
                            ResolveConflictDialog(
                                viewModel = tabViewModel(entry) { it.resolveConflictViewModelFactory().create(entry.path) },
                                onDismiss = { backStack.removeLastOrNull() },
                            )
                        }
                    }
                )
            }
        }

        if (processingTask != null) {
            ProcessingScreen(
                processingTask,
                onCancelOnGoingTask = { repositoryTabViewModel.cancelOngoingTask() }
            )
        }

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .align(Alignment.BottomCenter)
                .padding(bottom = 48.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            for (task in tasks) {

                val notificationData = task.toNotificationData()

                if (notificationData != null) {
                    Notification(notificationData)
                }

                LaunchedEffect(task) {
                    delay(2000L.milliseconds)
                    repositoryTabViewModel.completedTaskAlreadyShown(task)
                }
            }
        }
    }
}

fun CompletedTask.toNotificationData(): NotificationData? {
    val message = this.taskType.successTitle() ?: return null


    val type = when (this) {
        is CompletedTask.Failure -> NotificationType.Error
        is CompletedTask.Success -> NotificationType.Positive
    }


    return NotificationData(type, message)
}

private fun handleQuickLaunchCommand(
    command: QuickLaunchAction.Command,
    backStack: NavBackStack<Screen>,
    repositoryOpenViewModel: RepositoryOpenViewModel?,
) {
    val viewModel = repositoryOpenViewModel
    when (command.id) {
        "fetch" -> backStack.add(Screen.Fetch)
        "pull" -> backStack.add(Screen.Pull(null))
        "push" -> backStack.add(Screen.Push)
        "quick-fetch" -> viewModel?.fetchAll()
        "quick-pull" -> viewModel?.pull(PullType.MERGE)
        "quick-push" -> viewModel?.push(force = false, pushTags = false)
        "stash" -> viewModel?.stash()
        "create-branch" -> backStack.add(Screen.BranchCreate(null))
        "create-tag" -> {
            val commit = (viewModel?.selectedItem?.value as? SelectedItem.CommitBasedItem)?.commit
            if (commit != null) backStack.add(Screen.TagCreate(commit))
        }
        "terminal" -> viewModel?.openTerminal()
        "explorer" -> viewModel?.openInFileManager()
        "refresh" -> viewModel?.refreshAll()
        "preferences" -> backStack.add(Screen.Settings)
        "repository-settings" -> backStack.add(Screen.RepositorySettings)
        "git-flow-init" -> backStack.add(Screen.GitFlowInit)
        "git-flow-start" -> backStack.add(Screen.GitFlowStart)
        "git-flow-finish" -> backStack.add(Screen.GitFlowFinish)
        "file-history" -> if (command.argument.isNotEmpty()) viewModel?.fileHistory(command.argument)
    }
}
