package com.zhoujun.awegit.ui

import androidx.compose.desktop.ui.tooling.preview.Preview
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.DropdownMenu
import androidx.compose.material.DropdownMenuItem
import androidx.compose.material.Icon
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.pointer.isCtrlPressed
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.*
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import com.zhoujun.awegit.app.generated.resources.*
import com.zhoujun.awegit.domain.models.PullType
import com.zhoujun.awegit.extensions.handMouseClickable
import com.zhoujun.awegit.extensions.ignoreKeyEvents
import com.zhoujun.awegit.keybindings.Keybinding
import com.zhoujun.awegit.keybindings.KeybindingOption
import com.zhoujun.awegit.keybindings.keyBinding
import com.zhoujun.awegit.repositoryopen.RepositoryOpenViewModel
import com.zhoujun.awegit.theme.forkBorder
import com.zhoujun.awegit.theme.monoTypography
import com.zhoujun.awegit.theme.onBackgroundSecondary
import com.zhoujun.awegit.theme.toolbarBackground
import com.zhoujun.awegit.ui.components.fork.ForkToolbarButton
import com.zhoujun.awegit.ui.components.tooltip.InstantTooltip
import com.zhoujun.awegit.ui.context_menu.*
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource
import java.io.File

private const val DISABLED_BUTTON_ALPHA = 0.5F

@Composable
fun Menu(
    modifier: Modifier,
    viewModel: RepositoryOpenViewModel,
    onOpenAnotherRepository: (String) -> Unit,
    onQuickLaunch: () -> Unit,
    onShowSettingsDialog: () -> Unit,
    onRepositorySettings: () -> Unit,
    onPull: () -> Unit,
    onFetch: () -> Unit,
    onPush: () -> Unit,
    onGitFlowInit: () -> Unit,
    onGitFlowStart: () -> Unit,
    onGitFlowFinish: () -> Unit,
) {
    val isPullWithRebaseDefault by viewModel.isPullWithRebaseDefault.collectAsState(false)
    val hasUncommittedChanges by viewModel.hasUncommittedChanges.collectAsState()
    val stashesState by viewModel.stashesState.collectAsState()
    val status by viewModel.statusState.collectAsState()
    val branchLabel by viewModel.currentBranchLabel.collectAsState()
    val repositories by viewModel.workspaceRepositories.collectAsState()
    val keyboardModifiers = LocalWindowInfo.current.keyboardModifiers
    var showRepositories by remember { mutableStateOf(false) }
    var showGitFlow by remember { mutableStateOf(false) }
    val repositoryName = status.repositoryPath?.let { File(it).name }.orEmpty()

    Column(modifier) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(44.dp)
                .background(MaterialTheme.colors.toolbarBackground)
                .padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box {
                Column(
                    Modifier
                        .widthIn(max = 220.dp)
                        .clickable { showRepositories = true }
                        .padding(horizontal = 8.dp),
                ) {
                    Text(
                        repositoryName.ifEmpty { "Repository" },
                        fontSize = 13.sp,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                    )
                    Text(
                        branchLabel,
                        fontSize = 11.sp,
                        color = MaterialTheme.colors.onBackgroundSecondary,
                        maxLines = 1,
                    )
                }
                DropdownMenu(expanded = showRepositories, onDismissRequest = { showRepositories = false }) {
                    if (repositories.isEmpty()) {
                        DropdownMenuItem(onClick = { showRepositories = false }) {
                            Text("No repositories in this workspace")
                        }
                    }
                    for (path in repositories) {
                        DropdownMenuItem(onClick = {
                            showRepositories = false
                            onOpenAnotherRepository(path)
                        }) {
                            Column {
                                Text(File(path).name, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                                Text(path, fontSize = 11.sp, color = MaterialTheme.colors.onBackgroundSecondary, maxLines = 1)
                            }
                        }
                    }
                }
            }

            ForkToolbarButton(
                icon = Res.drawable.fetch,
                label = "Fetch",
                onClick = { if (keyboardModifiers.isCtrlPressed) viewModel.fetchAll() else onFetch() },
            )
            ForkToolbarButton(
                icon = Res.drawable.download,
                label = "Pull",
                onClick = {
                    if (keyboardModifiers.isCtrlPressed) {
                        val pullType = if (isPullWithRebaseDefault) PullType.REBASE else PullType.MERGE
                        viewModel.pull(pullType)
                    } else {
                        onPull()
                    }
                },
            )
            ForkToolbarButton(
                icon = Res.drawable.upload,
                label = "Push",
                onClick = {
                    if (keyboardModifiers.isCtrlPressed) viewModel.push(force = false, pushTags = false) else onPush()
                },
            )
            ForkToolbarButton(
                icon = Res.drawable.stash,
                label = "Stash",
                onClick = { if (hasUncommittedChanges) viewModel.stash() },
            )
            ForkToolbarButton(
                icon = Res.drawable.apply_stash,
                label = "Pop",
                onClick = { if (stashesState.stashes.isNotEmpty()) viewModel.popStash() },
            )
            Box {
                ForkToolbarButton(
                    icon = Res.drawable.branch,
                    label = "Git Flow",
                    onClick = { showGitFlow = true },
                )
                DropdownMenu(expanded = showGitFlow, onDismissRequest = { showGitFlow = false }) {
                    DropdownMenuItem(onClick = { showGitFlow = false; onGitFlowInit() }) { Text("Init") }
                    DropdownMenuItem(onClick = { showGitFlow = false; onGitFlowStart() }) { Text("Start") }
                    DropdownMenuItem(onClick = { showGitFlow = false; onGitFlowFinish() }) { Text("Finish") }
                }
            }

            Spacer(Modifier.weight(1f))

            Box(
                Modifier
                    .width(220.dp)
                    .height(24.dp)
                    .border(1.dp, MaterialTheme.colors.forkBorder, RoundedCornerShape(3.dp))
                    .clickable(onClick = onQuickLaunch)
                    .padding(horizontal = 8.dp),
                contentAlignment = Alignment.CenterStart,
            ) {
                Text("Quick Launch  Ctrl+P", fontSize = 12.sp, color = MaterialTheme.colors.onBackgroundSecondary, maxLines = 1)
            }

            ForkToolbarButton(icon = Res.drawable.terminal, label = "Terminal", onClick = viewModel::openTerminal)
            ForkToolbarButton(icon = Res.drawable.folder, label = "Explorer", onClick = viewModel::openInFileManager)
            ForkToolbarButton(icon = Res.drawable.settings, label = "Preferences", onClick = onShowSettingsDialog)
            ForkToolbarButton(icon = Res.drawable.list, label = "Repo", onClick = onRepositorySettings)
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(MaterialTheme.colors.forkBorder))
    }
}

fun MenuButton(
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    title: String,
    icon: Painter,
    keybinding: Keybinding?,
    tooltip: String,
    tooltipEnabled: Boolean = true,
    onClick: () -> Unit,
) {
    val keybinding = if (enabled) keybinding else null

    InstantTooltip(
        text = tooltip,
        enabled = tooltipEnabled,
        trailingContent = if (keybinding != null) {
            { KeybindingHint(keybinding) }
        } else {
            null
        }
    ) {
        val color = MaterialTheme.colors.onBackground.copy(alpha = if (enabled) 1F else DISABLED_BUTTON_ALPHA)

        Column(
            modifier = modifier
                .ignoreKeyEvents()
                .clip(RoundedCornerShape(4.dp))
                .background(MaterialTheme.colors.surface)
                .handMouseClickable(enabled) { onClick() }
                .size(56.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Icon(
                painter = icon,
                contentDescription = title,
                modifier = Modifier
                    .size(24.dp),
                tint = color,
            )
            Text(
                text = title,
                style = MaterialTheme.typography.caption,
                maxLines = 1,
                textAlign = TextAlign.Center,
                color = color,
            )
        }
    }
}

@Composable
fun KeybindingHint(keybinding: Keybinding) {
    val parts = remember(keybinding) { getParts(keybinding) }.joinToString("+")

    Text(
        parts,
        fontFamily = monoTypography(),
        fontSize = MaterialTheme.typography.caption.fontSize,
        fontWeight = FontWeight.Medium,
        color = MaterialTheme.colors.onBackgroundSecondary,
    )
}

@Preview
@Composable
fun KeybindingHintPartPreview() {
    KeybindingHintPart("CTRL")
}

@Composable
fun KeybindingHintPart(part: String) {
    Text(
        text = part,
        fontWeight = FontWeight.Medium,
        color = MaterialTheme.colors.primary,
        modifier = Modifier
            .clip(RoundedCornerShape(4.dp))
            .border(2.dp, MaterialTheme.colors.primary, RoundedCornerShape(4.dp))
            .background(MaterialTheme.colors.primary.copy(alpha = 0.05f))
            .padding(horizontal = 4.dp, vertical = 4.dp)

    )
}

fun getParts(keybinding: Keybinding): List<String> {
    val parts = mutableListOf<String>()

    if (keybinding.control) {
        parts.add("Ctrl")
    }

    if (keybinding.meta) {
        parts.add("鈱?)
    }

    if (keybinding.alt) {
        parts.add("Alt")
    }

    if (keybinding.shift) {
        parts.add("Shift")
    }

    val key = when (keybinding.key) {
        Key.A -> "A"
        Key.B -> "B"
        Key.C -> "C"
        Key.D -> "D"
        Key.E -> "E"
        Key.F -> "F"
        Key.G -> "G"
        Key.H -> "H"
        Key.I -> "I"
        Key.J -> "J"
        Key.K -> "K"
        Key.L -> "L"
        Key.M -> "M"
        Key.N -> "N"
        Key.O -> "O"
        Key.P -> "P"
        Key.Q -> "Q"
        Key.R -> "R"
        Key.S -> "S"
        Key.T -> "T"
        Key.U -> "U"
        Key.V -> "V"
        Key.W -> "W"
        Key.X -> "X"
        Key.Y -> "Y"
        Key.Z -> "Z"
        Key.Tab -> "Tab"
        else -> throw NotImplementedError("Key not implemented")
    }

    parts.add(key)

    return parts
}

@Composable
fun ExtendedMenuButton(
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    title: String,
    tooltipText: String,
    icon: Painter,
    keybinding: Keybinding?,
    onClick: () -> Unit,
    extendedListItems: List<ContextMenuElement>,
) {
    val color = MaterialTheme.colors.onBackground.copy(alpha = if (enabled) 1F else DISABLED_BUTTON_ALPHA)
    val keybinding = if (enabled) keybinding else null

    Row(
        modifier = modifier
            .size(width = 64.dp, height = 56.dp)
            .ignoreKeyEvents()
            .clip(RoundedCornerShape(4.dp))
            .background(MaterialTheme.colors.surface)
            .handMouseClickable(enabled) { onClick() }
    ) {
        InstantTooltip(
            text = tooltipText,
            modifier = Modifier
                .fillMaxHeight()
                .weight(1f),
            trailingContent = if (keybinding != null) {
                { KeybindingHint(keybinding) }
            } else {
                null
            }
        ) {
            Column(
                modifier = Modifier
                    .fillMaxSize(),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Icon(
                    painter = icon,
                    contentDescription = title,
                    modifier = Modifier
                        .size(24.dp),
                    tint = color,
                )
                Text(
                    text = title,
                    style = MaterialTheme.typography.caption,
                    color = color,
                    maxLines = 1,
                )
            }
        }

        DropDownMenu(
            enabled = enabled,
            items = { extendedListItems }
        ) {
            Box(
                modifier = Modifier
                    .fillMaxHeight()
                    .ignoreKeyEvents(),
                contentAlignment = Alignment.Center,
            ) {

                Icon(
                    painterResource(Res.drawable.expand_more),
                    contentDescription = null,
                    tint = color,
                    modifier = Modifier.size(16.dp)
                )

            }
        }
    }
}
