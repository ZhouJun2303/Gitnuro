@file:OptIn(ExperimentalComposeUiApi::class)

package com.zhoujun.awegit.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material.*
import androidx.compose.runtime.*
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.Alignment
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.LocalTabFocusRequester
import com.zhoujun.awegit.Screen
import com.zhoujun.awegit.app.generated.resources.*
import com.zhoujun.awegit.domain.extensions.isValid
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.Remote
import com.zhoujun.awegit.domain.models.Submodule
import com.zhoujun.awegit.domain.models.Tag
import com.zhoujun.awegit.domain.models.TrackingCounts
import com.zhoujun.awegit.domain.models.WorktreeInfo
import com.zhoujun.awegit.domain.models.ui.SelectedItem
import com.zhoujun.awegit.extensions.handOnHover
import com.zhoujun.awegit.extensions.setClipboardText
import com.zhoujun.awegit.repositoryopen.MainView
import com.zhoujun.awegit.repositoryopen.RepositoryOpenViewModel
import com.zhoujun.awegit.theme.backgroundSelected
import com.zhoujun.awegit.theme.onBackgroundSecondary
import com.zhoujun.awegit.theme.sidebarBackground
import com.zhoujun.awegit.ui.components.AdjustableOutlinedTextField
import com.zhoujun.awegit.ui.components.ScrollableLazyColumn
import com.zhoujun.awegit.ui.components.SideMenuHeader
import com.zhoujun.awegit.ui.components.SideMenuSubentry
import com.zhoujun.awegit.ui.components.tooltip.DelayedTooltip
import com.zhoujun.awegit.ui.context_menu.*
import com.zhoujun.awegit.viewmodels.sidepanel.*
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import org.eclipse.jgit.submodule.SubmoduleStatus
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource
import java.awt.datatransfer.StringSelection

@Composable
fun SidePanel(
    viewModel: RepositoryOpenViewModel,
    onNavigate: (Screen) -> Unit,
) {
    val filter by viewModel.filter.collectAsState()
    val selectedItem by viewModel.selectedItem.collectAsState()

    val branchesState by viewModel.branchesState.collectAsState()
    val remotesState by viewModel.remoteState.collectAsState()
    val tagsState by viewModel.tagsState.collectAsState()
    val stashesState by viewModel.stashesState.collectAsState()
    val submodulesState by viewModel.submodulesState.collectAsState()
    val changesCount by viewModel.changesCount.collectAsState()
    val mainView by viewModel.mainView.collectAsState()
    val hiddenRefs by viewModel.hiddenRefs.collectAsState()
    val tracking by viewModel.branchesTracking.collectAsState()
    val expandedFolders by viewModel.expandedBranchFolders.collectAsState()
    val worktrees by viewModel.worktrees.collectAsState()

    val searchFocusRequester = remember { FocusRequester() }
    val tabFocusRequester = LocalTabFocusRequester.current

    LaunchedEffect(viewModel) {
        viewModel.refreshWorktrees()
        viewModel.freeSearchFocusFlow.collectLatest {
            tabFocusRequester.requestFocus()
        }
    }

    Column(Modifier.fillMaxSize().background(MaterialTheme.colors.sidebarBackground)) {
        SidebarModeRow(
            text = "Local Changes ($changesCount)",
            selected = mainView == MainView.CHANGES,
            onClick = viewModel::showChanges,
        )
        SidebarModeRow(
            text = "All Commits",
            selected = mainView == MainView.ALL_COMMITS,
            onClick = viewModel::showAllCommits,
        )

        ScrollableLazyColumn(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .padding(top = 4.dp)
        ) {
            localBranches(
                branchesState = branchesState,
                selectedItem = selectedItem,
                viewModel = viewModel,
                hiddenRefs = hiddenRefs,
                tracking = tracking,
                expandedFolders = expandedFolders,
                forceExpand = filter.isNotBlank(),
                onChangeDefaultUpstreamBranch = { onNavigate(Screen.BranchChangeUpstream(it)) },
                onRenameBranch = { onNavigate(Screen.BranchRename(it)) },
                onNavigate = onNavigate,
            )

            remotes(
                remotesState = remotesState,
                viewModel = viewModel,
                expandedFolders = expandedFolders,
                forceExpand = filter.isNotBlank(),
                onShowAddEditRemoteDialog = { onNavigate(Screen.AddEditRemote(it)) },
                onNavigate = onNavigate,
            )

            tags(
                tagsState = tagsState,
                selectedItem = selectedItem,
                viewModel = viewModel,
            )

            stashes(
                stashesState = stashesState,
                selectedItem = selectedItem,
                viewModel = viewModel,
            )

            submodules(
                submodulesState = submodulesState,
                viewModel = viewModel,
                onAddSubmodule = { onNavigate(Screen.SubmoduleAdd) },
            )

            worktrees(worktrees = worktrees, viewModel = viewModel)
        }

        FilterTextField(
            value = filter,
            onValueChange = { newValue ->
                viewModel.newFilter(newValue)
            },
            modifier = Modifier
                .padding(start = 8.dp, bottom = 8.dp)
                .focusRequester(searchFocusRequester)
                .onFocusChanged {
                    if (it.isFocused) {
                        viewModel.addSidePanelSearchToCloseables()
                    } else {
                        viewModel.removeSidePanelSearchFromCloseables()
                    }
                }
        )
    }
}

@Composable
fun FilterTextField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier,
) {
    AdjustableOutlinedTextField(
        value = value,
        hint = stringResource(Res.string.side_pane_search_hint),
        onValueChange = onValueChange,
        modifier = modifier,
        textStyle = LocalTextStyle.current.copy(
            fontSize = MaterialTheme.typography.body2.fontSize,
            color = MaterialTheme.colors.onBackground,
        ),
        singleLine = true,
        leadingIcon = {
            Icon(
                painterResource(Res.drawable.search),
                contentDescription = null,
                modifier = Modifier.size(16.dp),
                tint = if (value.isEmpty()) MaterialTheme.colors.onBackgroundSecondary else MaterialTheme.colors.onBackground
            )
        },
        trailingIcon = {
            if (value.isNotEmpty()) {
                IconButton(
                    onClick = { onValueChange("") },
                    modifier = Modifier
                        .size(16.dp)
                        .handOnHover(),
                ) {
                    Icon(
                        painterResource(Res.drawable.close),
                        contentDescription = null,
                        tint = if (value.isEmpty()) MaterialTheme.colors.onBackgroundSecondary else MaterialTheme.colors.onBackground
                    )
                }
            }
        }
    )
}

@OptIn(ExperimentalComposeUiApi::class)
fun LazyListScope.localBranches(
    branchesState: BranchesState,
    selectedItem: SelectedItem,
    viewModel: RepositoryOpenViewModel,
    hiddenRefs: Set<String>,
    tracking: Map<String, TrackingCounts>,
    expandedFolders: Set<String>,
    forceExpand: Boolean,
    onChangeDefaultUpstreamBranch: (Branch) -> Unit,
    onRenameBranch: (Branch) -> Unit,
    onNavigate: (Screen) -> Unit,
) {
    val isExpanded = branchesState.isExpanded
    val branches = branchesState.branches
    val currentBranch = branchesState.currentBranch

    item {
        ContextMenu(
            items = { emptyList() }
        ) {
            SideMenuHeader(
                text = stringResource(Res.string.side_pane_local_branches_title),
                icon = painterResource(Res.drawable.branch),
                itemsCount = branches.count(),
                hoverIcon = null,
                isExpanded = isExpanded,
                onExpand = { viewModel.onExpandBranches() }
            )
        }
    }

    if (isExpanded) {
        refTree(
            keyPrefix = "local",
            nodes = buildRefTree(branches),
            depth = 0,
            expandedFolders = expandedFolders,
            forceExpand = forceExpand,
            onToggleFolder = viewModel::toggleBranchFolder,
        ) { branch, depth ->
            val scope = rememberCoroutineScope()
            val clipboard = LocalClipboard.current
            val counts = tracking[branch.simpleName]
            Branch(
                branch = branch,
                isSelectedItem = selectedItem is SelectedItem.BranchItem && selectedItem.branch == branch,
                currentBranch = currentBranch,
                depth = depth,
                hidden = branch.name in hiddenRefs,
                trackingText = counts?.let { "↑${it.ahead} ↓${it.behind}" },
                onToggleHidden = { viewModel.toggleHiddenRef(branch.name) },
                onBranchClicked = { viewModel.selectBranch(branch) },
                onBranchDoubleClicked = { viewModel.checkoutBranch(branch) },
                onCheckoutBranch = { viewModel.checkoutBranch(branch) },
                onMergeBranch = { onNavigate(Screen.Merge(branch)) },
                onRebaseBranch = { onNavigate(Screen.Rebase(branch)) },
                onDeleteBranch = { onNavigate(Screen.DeleteBranch(branch)) },
                onChangeDefaultUpstreamBranch = { onChangeDefaultUpstreamBranch(branch) },
                onRenameBranch = { onRenameBranch(branch) },
                onCopyBranchNameToClipboard = {
                    scope.launch { clipboard.setClipboardText(branch.simpleName) }
                },
            )
        }
    }
}

fun LazyListScope.remotes(
    remotesState: RemotesState,
    viewModel: RepositoryOpenViewModel,
    expandedFolders: Set<String>,
    forceExpand: Boolean,
    onShowAddEditRemoteDialog: (Remote?) -> Unit,
    onNavigate: (Screen) -> Unit,
) {
    val isExpanded = remotesState.isExpanded
    val remotes = remotesState.remotes

    item {
        SideMenuHeader(
            text = stringResource(Res.string.side_pane_remotes_title),
            icon = painterResource(Res.drawable.cloud),
            itemsCount = remotes.count(),
            hoverIcon = {
                IconButton(
                    onClick = { onShowAddEditRemoteDialog(null) },
                    modifier = Modifier
                        .padding(end = 16.dp)
                        .size(16.dp)
                        .handOnHover(),
                ) {
                    Icon(
                        painter = painterResource(Res.drawable.add),
                        contentDescription = null,
                        modifier = Modifier
                            .fillMaxSize(),
                        tint = MaterialTheme.colors.onBackground,
                    )
                }
            },
            isExpanded = isExpanded,
            onExpand = { viewModel.onExpandRemotes() }
        )
    }

    if (isExpanded) {
        for (remote in remotes) {
            item {
                Remote(
                    remote = remote,
                    onEditRemote = {
                        val wrapper = remote.remoteInfo.remote
                        onShowAddEditRemoteDialog(wrapper)
                    },
                    onDeleteRemote = { onNavigate(Screen.DeleteRemote(remote.remoteInfo)) },
                    onRemoteClicked = { viewModel.onRemoteClicked(remote) },
                    onFetchBranches = { viewModel.onFetchRemoteBranches(remote) },
                )
            }

            if (remote.isExpanded) {
                val remoteName = remote.remoteInfo.remote.name
                refTree(
                    keyPrefix = "remote-$remoteName",
                    nodes = buildRefTree(remote.remoteInfo.branchesList) { branch ->
                        branch.simpleName.removePrefix("$remoteName/")
                    },
                    depth = 1,
                    expandedFolders = expandedFolders,
                    forceExpand = forceExpand,
                    onToggleFolder = viewModel::toggleBranchFolder,
                ) { remoteBranch, depth ->
                    val scope = rememberCoroutineScope()
                    val clipboard = LocalClipboard.current
                    RemoteBranches(
                        remoteBranch = remoteBranch,
                        currentBranch = remotesState.currentBranch,
                        depth = depth,
                        onBranchClicked = { viewModel.selectBranch(remoteBranch) },
                        onCheckoutBranch = { onNavigate(Screen.CheckoutRemoteBranch(remoteBranch)) },
                        onDeleteBranch = { viewModel.deleteRemoteBranch(remoteBranch) },
                        onPushRemoteBranch = { viewModel.pushToRemoteBranch(remoteBranch) },
                        onPullRemoteBranch = { onNavigate(Screen.Pull(remoteBranch)) },
                        onRebaseRemoteBranch = { onNavigate(Screen.Rebase(remoteBranch)) },
                        onMergeRemoteBranch = { onNavigate(Screen.Merge(remoteBranch)) },
                        onCopyBranchNameToClipboard = {
                            scope.launch { clipboard.setClipboardText(remoteBranch.simpleName) }
                        },
                    )
                }
            }
        }
    }
}


fun LazyListScope.tags(
    tagsState: TagsState,
    viewModel: RepositoryOpenViewModel,
    selectedItem: SelectedItem,
) {
    val isExpanded = tagsState.isExpanded
    val tags = tagsState.tags

    item {
        ContextMenu(
            items = { emptyList() }
        ) {
            SideMenuHeader(
                text = stringResource(Res.string.side_pane_tags_title),
                icon = painterResource(Res.drawable.tag),
                itemsCount = tags.count(),
                hoverIcon = null,
                isExpanded = isExpanded,
                onExpand = { viewModel.onExpandTags() }
            )
        }
    }

    if (isExpanded) {
        items(tags, key = { it.name }) { tag ->
            Tag(
                tag,
                isSelected = selectedItem is SelectedItem.TagItem && selectedItem.tag == tag,
                onTagClicked = { viewModel.selectTag(tag) },
                onCheckoutTag = { viewModel.checkoutTagCommit(tag) },
                onDeleteTag = { onNavigate(Screen.DeleteTag(tag)) }
            )
        }
    }
}

fun LazyListScope.stashes(
    stashesState: StashesState,
    viewModel: RepositoryOpenViewModel,
    selectedItem: SelectedItem,
) {
    val isExpanded = stashesState.isExpanded
    val stashes = stashesState.stashes

    item {
        ContextMenu(
            items = { emptyList() }
        ) {
            SideMenuHeader(
                text = stringResource(Res.string.side_pane_stashes_title),
                icon = painterResource(Res.drawable.stash),
                itemsCount = stashes.count(),
                hoverIcon = null,
                isExpanded = isExpanded,
                onExpand = { viewModel.onExpandStashes() }
            )
        }
    }

    if (isExpanded) {
        items(stashes, key = { it.hash }) { stash ->
            Stash(
                stash,
                isSelected = selectedItem is SelectedItem.CommitItem && selectedItem.isStash && selectedItem.commit.hash == stash.hash,
                onClick = { viewModel.selectStash(stash) },
                onApply = { viewModel.applyStash(stash) },
                onPop = { viewModel.popStash(stash) },
                onDelete = { onNavigate(Screen.DeleteStash(stash)) },
            )
        }
    }
}

fun LazyListScope.submodules(
    submodulesState: SubmodulesState,
    viewModel: RepositoryOpenViewModel,
    onAddSubmodule: () -> Unit,
) {
    val isExpanded = submodulesState.isExpanded
    val submodules = submodulesState.submodules

    item {
        ContextMenu(
            items = { emptyList() }
        ) {
            SideMenuHeader(
                text = stringResource(Res.string.side_pane_submodules_title),
                icon = painterResource(Res.drawable.topic),
                itemsCount = submodules.count(),
                hoverIcon = {
                    IconButton(
                        onClick = onAddSubmodule,
                        modifier = Modifier
                            .padding(end = 16.dp)
                            .size(16.dp)
                            .handOnHover(),
                    ) {
                        Icon(
                            painter = painterResource(Res.drawable.add),
                            contentDescription = null,
                            modifier = Modifier
                                .fillMaxSize(),
                            tint = MaterialTheme.colors.onBackground,
                        )
                    }
                },
                isExpanded = isExpanded,
                onExpand = { viewModel.onExpandSubmodules() }
            )
        }
    }

    if (isExpanded) {
        items(submodules, key = { it.first }) { submodule ->
            Submodule(
                submodule = submodule,
                onInitializeSubmodule = { viewModel.initializeSubmodule(submodule.first) },
//                onDeinitializeSubmodule = { submodulesViewModel.onDeinitializeSubmodule(submodule.first) },
                onSyncSubmodule = { viewModel.syncSubmodule(submodule.first) },
                onUpdateSubmodule = { viewModel.updateSubmodule(submodule.first) },
                onOpenSubmoduleInTab = { viewModel.onOpenSubmoduleInTab(submodule.first) },
                onDeleteSubmodule = { viewModel.deleteSubmodule(submodule.first) },
            )
        }
    }
}

@Composable
private fun Branch(
    branch: Branch,
    currentBranch: Branch?,
    isSelectedItem: Boolean,
    depth: Int = 0,
    hidden: Boolean = false,
    trackingText: String? = null,
    onToggleHidden: () -> Unit = {},
    onBranchClicked: () -> Unit,
    onBranchDoubleClicked: () -> Unit,
    onCheckoutBranch: () -> Unit,
    onMergeBranch: () -> Unit,
    onRebaseBranch: () -> Unit,
    onDeleteBranch: () -> Unit,
    onChangeDefaultUpstreamBranch: () -> Unit,
    onRenameBranch: () -> Unit,
    onCopyBranchNameToClipboard: () -> Unit,
) {
    val isCurrentBranch = currentBranch?.name == branch.name

    ContextMenu(
        items = {
            branchContextMenuItems(
                branch = branch,
                currentBranch = currentBranch,
                isCurrentBranch = isCurrentBranch,
                isLocal = true,
                onCheckoutBranch = onCheckoutBranch,
                onMergeBranch = onMergeBranch,
                onDeleteBranch = onDeleteBranch,
                onRebaseBranch = onRebaseBranch,
                onPushToRemoteBranch = {},
                onPullFromRemoteBranch = {},
                onChangeDefaultUpstreamBranch = onChangeDefaultUpstreamBranch,
                onRenameBranch = onRenameBranch,
                onCopyBranchNameToClipboard = onCopyBranchNameToClipboard
            )
        }
    ) {
        SideMenuSubentry(
            text = if (isCurrentBranch) "✓ ${branch.simpleName.substringAfterLast('/')}" else branch.simpleName.substringAfterLast('/'),
            fontWeight = if (isCurrentBranch) FontWeight.Bold else FontWeight.Normal,
            iconResourcePath = if (hidden) Res.drawable.visibility_off else Res.drawable.visibility,
            extraPadding = (depth * 16).dp,
            isSelected = isSelectedItem,
            onClick = onBranchClicked,
            onDoubleClick = onBranchDoubleClicked,
        ) {
            IconButton(
                onClick = onToggleHidden,
                modifier = Modifier.size(16.dp).padding(end = 4.dp),
            ) {
                Icon(
                    painterResource(if (hidden) Res.drawable.visibility_off else Res.drawable.visibility),
                    contentDescription = null,
                    modifier = Modifier.size(12.dp),
                    tint = MaterialTheme.colors.onBackgroundSecondary,
                )
            }
            if (trackingText != null) {
                Text(
                    trackingText,
                    color = MaterialTheme.colors.onBackgroundSecondary,
                    fontSize = 11.sp,
                    modifier = Modifier.padding(end = 8.dp),
                )
            }
        }
    }
}


@Composable
private fun Remote(
    remote: RemoteView,
    onEditRemote: () -> Unit,
    onDeleteRemote: () -> Unit,
    onFetchBranches: () -> Unit,
    onRemoteClicked: () -> Unit,
) {
    ContextMenu(
        items = {
            remoteContextMenu(
                onEdit = onEditRemote,
                onDelete = onDeleteRemote,
                onFetch = onFetchBranches,
            )
        }
    ) {
        SideMenuSubentry(
            text = remote.remoteInfo.remote.name,
            iconResourcePath = Res.drawable.cloud,
            onClick = onRemoteClicked,
            isSelected = false,
        )
    }
}


@Composable
private fun RemoteBranches(
    remoteBranch: Branch,
    currentBranch: Branch?,
    depth: Int = 1,
    onBranchClicked: () -> Unit,
    onCheckoutBranch: () -> Unit,
    onDeleteBranch: () -> Unit,
    onPushRemoteBranch: () -> Unit,
    onPullRemoteBranch: () -> Unit,
    onRebaseRemoteBranch: () -> Unit,
    onMergeRemoteBranch: () -> Unit,
    onCopyBranchNameToClipboard: () -> Unit,
) {
    ContextMenu(
        items = {
            branchContextMenuItems(
                branch = remoteBranch,
                currentBranch = currentBranch,
                isCurrentBranch = false,
                isLocal = false,
                onCheckoutBranch = onCheckoutBranch,
                onMergeBranch = onMergeRemoteBranch,
                onDeleteBranch = {},
                onDeleteRemoteBranch = onDeleteBranch,
                onRebaseBranch = onRebaseRemoteBranch,
                onPushToRemoteBranch = onPushRemoteBranch,
                onPullFromRemoteBranch = onPullRemoteBranch,
                onChangeDefaultUpstreamBranch = {},
                onRenameBranch = {},
                onCopyBranchNameToClipboard = onCopyBranchNameToClipboard
            )
        }
    ) {
        SideMenuSubentry(
            text = remoteBranch.simpleName,
            extraPadding = (depth * 16).dp,
            isSelected = false,
            iconResourcePath = Res.drawable.branch,
            onClick = onBranchClicked,
            onDoubleClick = onCheckoutBranch,
        )
    }
}

@Composable
private fun Tag(
    tag: Tag,
    isSelected: Boolean,
    onTagClicked: () -> Unit,
    onCheckoutTag: () -> Unit,
    onDeleteTag: () -> Unit,
) {
    ContextMenu(
        items = {
            tagContextMenuItems(
                onCheckoutTag = onCheckoutTag,
                onDeleteTag = onDeleteTag,
            )
        }
    ) {
        SideMenuSubentry(
            text = tag.simpleName,
            isSelected = isSelected,
            iconResourcePath = Res.drawable.tag,
            onClick = onTagClicked,
        )
    }
}


@Composable
private fun Stash(
    stash: Commit,
    isSelected: Boolean,
    onClick: () -> Unit,
    onApply: () -> Unit,
    onPop: () -> Unit,
    onDelete: () -> Unit,
) {
    ContextMenu(
        items = {
            stashesContextMenuItems(
                onApply = onApply,
                onPop = onPop,
                onDelete = onDelete,
            )
        }
    ) {
        SideMenuSubentry(
            text = stash.shortMessage,
            isSelected = isSelected,
            iconResourcePath = Res.drawable.stash,
            onClick = onClick,
        )
    }
}

@Composable
private fun Submodule(
    submodule: Pair<String, Submodule>,
    onInitializeSubmodule: () -> Unit,
//    onDeinitializeSubmodule: () -> Unit,
    onSyncSubmodule: () -> Unit,
    onUpdateSubmodule: () -> Unit,
    onOpenSubmoduleInTab: () -> Unit,
    onDeleteSubmodule: () -> Unit,
) {
    ContextMenu(
        items = {
            submoduleContextMenuItems(
                submodule.second,
                onInitializeSubmodule = onInitializeSubmodule,
//                onDeinitializeSubmodule = onDeinitializeSubmodule,
                onSyncSubmodule = onSyncSubmodule,
                onUpdateSubmodule = onUpdateSubmodule,
                onOpenSubmoduleInTab = onOpenSubmoduleInTab,
                onDeleteSubmodule = onDeleteSubmodule,
            )
        }
    ) {
        SideMenuSubentry(
            text = submodule.first,
            iconResourcePath = Res.drawable.topic,
            isSelected = false,
            onClick = {
                if (submodule.second.state.isValid) {
                    onOpenSubmoduleInTab()
                }
            },
        ) {
            val stateName = submodule.second.state.toString()
            DelayedTooltip(stateName) {
                Text(
                    text = stateName.first().toString(),
                    color = MaterialTheme.colors.onBackgroundSecondary,
                    style = MaterialTheme.typography.body2,
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }
        }
    }
}

@Composable
private fun SidebarModeRow(text: String, selected: Boolean, onClick: () -> Unit) {
    Text(
        text,
        fontSize = 12.sp,
        fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
        modifier = Modifier
            .fillMaxWidth()
            .height(22.dp)
            .background(if (selected) MaterialTheme.colors.backgroundSelected else MaterialTheme.colors.sidebarBackground)
            .clickable(onClick = onClick)
            .padding(start = 12.dp, top = 2.dp),
        maxLines = 1,
    )
}

private fun LazyListScope.refTree(
    keyPrefix: String,
    nodes: List<RefTreeNode>,
    depth: Int,
    expandedFolders: Set<String>,
    forceExpand: Boolean,
    onToggleFolder: (String) -> Unit,
    leaf: @Composable (Branch, Int) -> Unit,
) {
    for (node in nodes) {
        when (node) {
            is RefTreeNode.Folder -> {
                val open = forceExpand || node.path in expandedFolders
                item(key = "$keyPrefix-folder-$depth-${node.path}") {
                    Text(
                        text = if (open) "▾  ${node.name}" else "▸  ${node.name}",
                        fontSize = 12.sp,
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(22.dp)
                            .clickable { onToggleFolder(node.path) }
                            .padding(start = (12 + depth * 16).dp, top = 2.dp),
                        maxLines = 1,
                    )
                }
                if (open) {
                    refTree(keyPrefix, node.children, depth + 1, expandedFolders, forceExpand, onToggleFolder, leaf)
                }
            }
            is RefTreeNode.Leaf -> item(key = "$keyPrefix-${node.branch.name}") {
                leaf(node.branch, depth)
            }
        }
    }
}

private fun LazyListScope.worktrees(
    worktrees: com.zhoujun.awegit.domain.models.WorktreeListResult,
    viewModel: RepositoryOpenViewModel,
) {
    item {
        SideMenuHeader(
            text = "Worktrees",
            icon = painterResource(Res.drawable.folder),
            itemsCount = worktrees.worktrees.size,
            hoverIcon = null,
            isExpanded = true,
            onExpand = {},
        )
    }
    if (!worktrees.gitAvailable) {
        item {
            Text(
                "Requires Git command line",
                fontSize = 12.sp,
                color = MaterialTheme.colors.onBackgroundSecondary,
                modifier = Modifier.padding(start = 32.dp, top = 4.dp),
            )
        }
        return
    }
    items(worktrees.worktrees, key = { "worktree-${it.path}" }) { worktree ->
        ContextMenu(
            items = {
                listOf(
                    ContextMenuElement.ContextTextEntry("Open", onClick = { viewModel.openWorktree(worktree.path) }),
                    ContextMenuElement.ContextTextEntry("Show in File Manager", onClick = { viewModel.showWorktree(worktree.path) }),
                    ContextMenuElement.ContextTextEntry("Remove", onClick = { viewModel.removeWorktree(worktree.path) }),
                )
            }
        ) {
            SideMenuSubentry(
                text = "${worktree.branch}  ${worktree.path}",
                iconResourcePath = Res.drawable.folder,
                isSelected = false,
                onClick = { viewModel.openWorktree(worktree.path) },
            )
        }
    }
}