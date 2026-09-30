package com.zhoujun.awegit.repositoryopen

import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.*
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import com.zhoujun.awegit.LocalTabFocusRequester
import com.zhoujun.awegit.Screen
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.bottom_info_bar_email_not_set
import com.zhoujun.awegit.app.generated.resources.bottom_info_bar_name_and_email
import com.zhoujun.awegit.app.generated.resources.bottom_info_bar_name_not_set
import com.zhoujun.awegit.domain.models.Identity
import com.zhoujun.awegit.domain.models.PullType
import com.zhoujun.awegit.domain.models.RebaseInteractiveState
import com.zhoujun.awegit.domain.models.RepositoryState
import com.zhoujun.awegit.domain.models.ui.SelectedItem
import com.zhoujun.awegit.extensions.handMouseClickable
import com.zhoujun.awegit.keybindings.KeybindingOption
import com.zhoujun.awegit.keybindings.matchesBinding
import com.zhoujun.awegit.ui.*
import com.zhoujun.awegit.ui.components.BottomInfoBar
import com.zhoujun.awegit.ui.components.TripleVerticalSplitPanel
import com.zhoujun.awegit.ui.components.fork.ForkSplitPane
import com.zhoujun.awegit.ui.diff.DiffPane
import com.zhoujun.awegit.ui.log.Log
import com.zhoujun.awegit.ui.status.StatusPane
import com.zhoujun.awegit.updates.Update
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.stringResource

@Composable
fun RepositoryOpenPage(
    repositoryOpenViewModel: RepositoryOpenViewModel,
    onNavigate: (Screen) -> Unit, // TODO Perhaps have specific callbacks instead of directly navigating
) {
    val repositoryState by repositoryOpenViewModel.repositoryState.collectAsState()
    val selectedItem by repositoryOpenViewModel.selectedItem.collectAsState()
    val blameState by repositoryOpenViewModel.blameState.collectAsState()
    val showHistory by repositoryOpenViewModel.showHistory.collectAsState()

    val focusRequester = remember { FocusRequester() }

    Column(
        modifier = Modifier
            .focusRequester(focusRequester)
            .focusable(true)
            .onPreviewKeyEvent {
                when {
                    it.matchesBinding(KeybindingOption.QUICK_LAUNCH) -> {
                        onNavigate(Screen.QuickLaunch)
                        true
                    }

                    it.matchesBinding(KeybindingOption.PULL) -> {
                        onNavigate(Screen.Pull(null))
                        true
                    }

                    it.matchesBinding(KeybindingOption.PUSH) -> {
                        onNavigate(Screen.Push)
                        true
                    }

                    it.matchesBinding(KeybindingOption.QUICK_PUSH) -> {
                        repositoryOpenViewModel.push(force = false, pushTags = false)
                        true
                    }

                    it.matchesBinding(KeybindingOption.FETCH) -> {
                        onNavigate(Screen.Fetch)
                        true
                    }

                    it.matchesBinding(KeybindingOption.QUICK_FETCH) -> {
                        repositoryOpenViewModel.fetchAll()
                        true
                    }

                    it.matchesBinding(KeybindingOption.QUICK_PULL) -> {
                        repositoryOpenViewModel.pull(com.zhoujun.awegit.domain.models.PullType.MERGE)
                        true
                    }

                    it.matchesBinding(KeybindingOption.SHOW_CHANGES) -> {
                        repositoryOpenViewModel.showChanges()
                        true
                    }

                    it.matchesBinding(KeybindingOption.SHOW_ALL_COMMITS) -> {
                        repositoryOpenViewModel.showAllCommits()
                        true
                    }

                    it.matchesBinding(KeybindingOption.BRANCH_CREATE) -> {
                        onNavigate(Screen.BranchCreate(null))
                        true
                    }

                    it.matchesBinding(KeybindingOption.STASH) -> {
                        repositoryOpenViewModel.stash()
                        true
                    }

                    it.matchesBinding(KeybindingOption.STASH_POP) -> {
                        repositoryOpenViewModel.popStash()
                        true
                    }

                    it.matchesBinding(KeybindingOption.EXIT) -> {
                        repositoryOpenViewModel.closeLastView()
                        true
                    }

                    it.matchesBinding(KeybindingOption.REFRESH) -> {
                        repositoryOpenViewModel.refreshAll()
                        true
                    }

                    it.matchesBinding(KeybindingOption.OPEN_REPOSITORY) -> {
                        val repoToOpen = repositoryOpenViewModel.openDirectoryPicker()
                        if (repoToOpen != null) repositoryOpenViewModel.openAnotherRepository(repoToOpen)
                        true
                    }

                    it.matchesBinding(KeybindingOption.SETTINGS) -> {
                        onNavigate(Screen.Settings)
                        true
                    }

                    else -> false
                }

            }
    ) {
        CompositionLocalProvider(
            LocalTabFocusRequester provides focusRequester
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Menu(
                    viewModel = repositoryOpenViewModel,
                    modifier = Modifier.fillMaxWidth(),
                    onOpenAnotherRepository = { repositoryOpenViewModel.openAnotherRepository(it) },
                    onQuickLaunch = { onNavigate(Screen.QuickLaunch) },
                    onShowSettingsDialog = { onNavigate(Screen.Settings) },
                    onRepositorySettings = { onNavigate(Screen.RepositorySettings) },
                    onPull = { onNavigate(Screen.Pull(null)) },
                    onFetch = { onNavigate(Screen.Fetch) },
                    onPush = { onNavigate(Screen.Push) },
                    onGitFlowInit = { onNavigate(Screen.GitFlowInit) },
                    onGitFlowStart = { onNavigate(Screen.GitFlowStart) },
                    onGitFlowFinish = { onNavigate(Screen.GitFlowFinish) },
                )

                RepoContent(
                    repositoryOpenViewModel = repositoryOpenViewModel,
                    selectedItem = selectedItem,
                    repositoryState = repositoryState,
                    blameState = blameState,
                    showHistory = showHistory,
                    onNavigate = onNavigate,
                )
            }
        }

        Spacer(
            modifier = Modifier
                .height(1.dp)
                .fillMaxWidth()
                .background(MaterialTheme.colors.primaryVariant.copy(alpha = 0.2f))
        )


        val userInfo by repositoryOpenViewModel.authorInfoSimple.collectAsState()
        val newUpdate = repositoryOpenViewModel.update.collectAsState().value

        RepositoryOpenBottomInfoBar(
            userInfo,
            newUpdate,
            onOpenUrlInBrowser = { repositoryOpenViewModel.openUrlInBrowser(it) },
            onShowAuthorInfoDialog = { onNavigate(Screen.Author) },
        )
    }

    LaunchedEffect(repositoryOpenViewModel) {
        focusRequester.requestFocus()
    }
}

@Composable
private fun RepositoryOpenBottomInfoBar(
    userInfo: UiDataState<Identity>,
    newUpdate: Update?,
    onOpenUrlInBrowser: (String) -> Unit,
    onShowAuthorInfoDialog: () -> Unit,
) {
    BottomInfoBar(
        newUpdate,
        onOpenUrlInBrowser,
        leadingContent = {
            Box(
                modifier = Modifier
                    .fillMaxHeight()
                    .handMouseClickable { onShowAuthorInfoDialog() },
                contentAlignment = Alignment.Center,
            ) {
                val identity = userInfo.data

                if (identity != null) {
                    val name = identity.name ?: stringResource(Res.string.bottom_info_bar_name_not_set)
                    val email = identity.email ?: stringResource(Res.string.bottom_info_bar_email_not_set)

                    Text(
                        text = stringResource(Res.string.bottom_info_bar_name_and_email, name, email),
                        style = MaterialTheme.typography.body2,
                        color = MaterialTheme.colors.onBackground,
                    )
                }
            }
        }
    )
}

@Composable
fun RepoContent(
    repositoryOpenViewModel: RepositoryOpenViewModel,
    selectedItem: SelectedItem,
    repositoryState: RepositoryState,
    blameState: BlameState,
    showHistory: Boolean,
    onNavigate: (Screen) -> Unit,
) {
    if (showHistory) {
        val historyViewModel = repositoryOpenViewModel.historyViewModel

        if (historyViewModel != null) {
            FileHistory(
                historyViewModel = historyViewModel,
                onClose = {
                    repositoryOpenViewModel.closeHistory()
                }
            )
        }
    } else {
        MainContentView(
            viewModel = repositoryOpenViewModel,
            selectedItem = selectedItem,
            repositoryState = repositoryState,
            blameState = blameState,
            onNavigate = onNavigate,
        )
    }
}

@Composable
fun MainContentView(
    viewModel: RepositoryOpenViewModel,
    selectedItem: SelectedItem,
    repositoryState: RepositoryState,
    blameState: BlameState,
    onNavigate: (Screen) -> Unit,
) {
    val diffSelected by viewModel.diffSelected.collectAsState()
    val rebaseInteractiveState by viewModel.rebaseInteractiveState.collectAsState()
    val density = LocalDensity.current.density
    val scope = rememberCoroutineScope()

    val statusState by viewModel.statusState.collectAsState()
    val mainView by viewModel.mainView.collectAsState()
    var commitsRatio by remember(viewModel) { mutableStateOf(0.55f) }
    var changesRatio by remember(viewModel) { mutableStateOf(0.42f) }

    // We create 2 mutableStates here because using directly the flow makes compose lose some drag events for some reason
    var firstWidth by remember(viewModel) { mutableStateOf(viewModel.firstPaneWidth.value) }
    var thirdWidth by remember(viewModel) { mutableStateOf(viewModel.thirdPaneWidth.value) }

    LaunchedEffect(Unit) {
        // Update the pane widths if they have been changed in a different tab
        viewModel.onPanelsWidthPersisted.collectLatest {
            firstWidth = viewModel.firstPaneWidth.value
            thirdWidth = viewModel.thirdPaneWidth.value
        }
    }

    TripleVerticalSplitPanel(
        modifier = Modifier.fillMaxSize(),
        firstWidth = if (rebaseInteractiveState is RebaseInteractiveState.AwaitingInteraction) 0f else firstWidth,
        thirdWidth = 0f,
        first = {
            SidePanel(
                viewModel = viewModel,
                onNavigate = onNavigate,
            )
        },
        second = {
            Box(
                modifier = Modifier
                    .fillMaxSize()
            ) {
                if (
                    rebaseInteractiveState is RebaseInteractiveState.AwaitingInteraction
                    && diffSelected == null
                ) {
                    RebaseInteractive(viewModel, rebaseInteractiveState)
                } else if (blameState is BlameState.Loaded && !blameState.isMinimized) {
                    Blame(
                        filePath = blameState.filePath,
                        blameResult = blameState.blameResult,
                        onClose = { viewModel.resetBlameState() },
                        onSelectCommit = { viewModel.selectCommit(it) }
                    )
                } else if (mainView == MainView.CHANGES) {
                    ForkSplitPane(
                        ratio = changesRatio,
                        onRatioChange = { changesRatio = it },
                        modifier = Modifier.fillMaxSize(),
                        horizontal = false,
                        first = {
                            StatusPane(
                                statusState = statusState,
                                completedTasks = viewModel.completedTasks,
                                onAction = { viewModel.onAction(it) },
                                onBlameFile = { viewModel.blameFile(it) },
                                onHistoryFile = { viewModel.fileHistory(it) },
                                onResolveConflict = { onNavigate(Screen.ResolveConflict(it)) },
                            )
                        },
                        second = {
                            if (diffSelected?.entries?.count() == 1) {
                                val tabFocusRequester = LocalTabFocusRequester.current
                                DiffPane(
                                    viewModel = viewModel,
                                    onCloseDiffView = { tabFocusRequester.requestFocus() },
                                )
                            }
                        },
                    )
                } else {
                    val commitChangesState = viewModel.commitChangesState.collectAsState().value
                    ForkSplitPane(
                        ratio = commitsRatio,
                        onRatioChange = { commitsRatio = it },
                        modifier = Modifier.fillMaxSize(),
                        first = {
                            Log(
                                viewModel = viewModel,
                                selectedItem = selectedItem,
                                repositoryState = repositoryState,
                                onCreateBranch = { onNavigate(Screen.BranchCreate(it)) },
                                onResetBranch = { onNavigate(Screen.BranchReset(it)) },
                                onCreateTag = { onNavigate(Screen.TagCreate(it)) },
                                onChangeUpstreamBranch = { onNavigate(Screen.BranchChangeUpstream(it)) },
                                onRenameBranch = { onNavigate(Screen.BranchRename(it)) },
                                onPullFromRemoteBranch = { onNavigate(Screen.Pull(it)) },
                                onRewordCommit = { onNavigate(Screen.RewordCommit(it)) },
                                onMergeBranch = { onNavigate(Screen.Merge(it)) },
                                onRebaseBranch = { onNavigate(Screen.Rebase(it)) },
                                onCheckoutRemoteBranch = { onNavigate(Screen.CheckoutRemoteBranch(it)) },
                                onDeleteBranch = { onNavigate(Screen.DeleteBranch(it)) },
                                onDeleteTag = { onNavigate(Screen.DeleteTag(it)) },
                                onDeleteStash = { onNavigate(Screen.DeleteStash(it)) },
                            )
                        },
                        second = {
                            Column(Modifier.fillMaxSize()) {
                                Box(Modifier.weight(1f).fillMaxWidth()) {
                                    if (commitChangesState != null) {
                                        CommitChanges(
                                            viewModel = viewModel,
                                            onBlame = { viewModel.blameFile(it) },
                                            onHistory = { viewModel.fileHistory(it) },
                                            commitChangesState = commitChangesState,
                                        )
                                    }
                                }
                                if (blameState is BlameState.Loaded) {
                                    MinimizedBlame(
                                        filePath = blameState.filePath,
                                        onExpand = { viewModel.expandBlame() },
                                        onClose = { viewModel.resetBlameState() },
                                    )
                                }
                            }
                        },
                    )
                }
            }
        },
        third = {},
        onFirstSizeDragStarted = { currentWidth ->
            firstWidth = currentWidth
            viewModel.setFirstPaneWidth(currentWidth)
        },
        onFirstSizeChange = {
            val newWidth = firstWidth + it / density

            if (newWidth > 150 && rebaseInteractiveState !is RebaseInteractiveState.AwaitingInteraction) {
                firstWidth = newWidth
                viewModel.setFirstPaneWidth(newWidth)
            }
        },
        onFirstSizeDragStopped = {
            scope.launch {
                viewModel.persistFirstPaneWidth()
            }
        },
        onThirdSizeChange = {
            val newWidth = thirdWidth - it / density

            if (newWidth > 150) {
                thirdWidth = newWidth
                viewModel.setThirdPaneWidth(newWidth)
            }
        },
        onThirdSizeDragStarted = { currentWidth ->
            thirdWidth = currentWidth
            viewModel.setThirdPaneWidth(currentWidth)
        },
        onThirdSizeDragStopped = {
            scope.launch {
                viewModel.persistThirdPaneWidth()
            }
        },
    )
}

