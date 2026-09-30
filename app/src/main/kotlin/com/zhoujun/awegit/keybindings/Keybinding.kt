@file:OptIn(ExperimentalComposeUiApi::class)

package com.zhoujun.awegit.keybindings

import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.input.key.*
import com.zhoujun.awegit.common.OS
import com.zhoujun.awegit.common.currentOs

data class Keybinding(
    val alt: Boolean = false,
    val control: Boolean = false,
    val meta: Boolean = false,
    val shift: Boolean = false,
    val key: Key,
)

enum class KeybindingOption {
    REFRESH,

    /**
     * Used mostly for dialogs with a single input field
     */
    SIMPLE_ACCEPT,

    /**
     * Used to accept multi-line text field like the commit message
     */
    TEXT_ACCEPT,

    /**
     * Used to close dialogs or components
     */
    EXIT,

    /**
     * Used to go up in lists
     */
    UP,

    /**
     * Used to go down in lists
     */
    DOWN,

    /**
     * Used to pull in current repository
     */
    PULL,

    /**
     * Used to push in current repository
     */
    PUSH,

    /**
     * Used to show branch creation dialog
     */
    BRANCH_CREATE,

    /**
     * Used to stash workspace changes
     */
    STASH,

    /**
     * Used to pop stash changes to workspace
     */
    STASH_POP,

    /**
     * Used to open a repository
     */
    OPEN_REPOSITORY,

    /**
     * Used to open a new tab
     */
    OPEN_NEW_TAB,

    /**
     * Used to close current tab
     */
    CLOSE_CURRENT_TAB,

    /**
     * Used to change current tab to the one in the left
     */
    CHANGE_CURRENT_TAB_LEFT,

    /**
     * Used to change current tab to the one in the right
     */
    CHANGE_CURRENT_TAB_RIGHT,

    /**
     * Used to open the settings screen
     */
    SETTINGS,
    QUICK_LAUNCH,
    FETCH,
    QUICK_FETCH,
    QUICK_PULL,
    QUICK_PUSH,
    TAG_CREATE,
    CLONE,
    INIT_REPOSITORY,
    SHOW_CHANGES,
    SHOW_ALL_COMMITS,
    REVEAL_HEAD,
    ZOOM_IN,
    ZOOM_OUT,
    COMMIT_SEARCH,
    COMMIT_AND_PUSH,
    STAGE_TOGGLE_SELECTED,
    STAGE_TOGGLE_ALL,
    DISCARD_SELECTED,
    OPEN_IN_FILE_MANAGER,
    OPEN_IN_TERMINAL,
    FILTER_ACTIVE_BRANCH,
}


@OptIn(ExperimentalComposeUiApi::class)
private fun baseKeybindings() = mapOf(
    KeybindingOption.REFRESH to listOf(
        Keybinding(key = Key.F5),
        Keybinding(control = true, key = Key.R),
    ),
    KeybindingOption.SIMPLE_ACCEPT to listOf(
        Keybinding(key = Key.Enter),
    ),
    KeybindingOption.TEXT_ACCEPT to listOf(
        Keybinding(control = true, key = Key.Enter),
    ),
    KeybindingOption.EXIT to listOf(
        Keybinding(key = Key.Escape),
    ),
    KeybindingOption.UP to listOf(
        Keybinding(key = Key.DirectionUp),
    ),
    KeybindingOption.DOWN to listOf(
        Keybinding(key = Key.DirectionDown),
    ),
    KeybindingOption.PULL to listOf(
        Keybinding(key = Key.L, control = true, shift = true),
    ),
    KeybindingOption.PUSH to listOf(
        Keybinding(key = Key.P, control = true, shift = true),
    ),
    KeybindingOption.BRANCH_CREATE to listOf(
        Keybinding(key = Key.B, control = true, shift = true),
    ),
    KeybindingOption.STASH to listOf(
        Keybinding(key = Key.H, control = true, shift = true),
    ),
    KeybindingOption.STASH_POP to emptyList(),
    KeybindingOption.OPEN_REPOSITORY to listOf(
        Keybinding(key = Key.O, control = true),
    ),
    KeybindingOption.OPEN_NEW_TAB to listOf(
        Keybinding(key = Key.T, control = true),
    ),
    KeybindingOption.CLOSE_CURRENT_TAB to listOf(
        Keybinding(key = Key.W, control = true),
    ),
    KeybindingOption.CHANGE_CURRENT_TAB_LEFT to listOf(
        Keybinding(key = Key.Tab, control = true, shift = true),
    ),
    KeybindingOption.CHANGE_CURRENT_TAB_RIGHT to listOf(
        Keybinding(key = Key.Tab, control = true),
    ),
    KeybindingOption.SETTINGS to listOf(
        Keybinding(key = Key.Comma, control = true),
    ),
    KeybindingOption.QUICK_LAUNCH to listOf(Keybinding(key = Key.P, control = true)),
    KeybindingOption.FETCH to listOf(Keybinding(key = Key.F, control = true, shift = true)),
    KeybindingOption.QUICK_FETCH to listOf(Keybinding(key = Key.F, control = true, alt = true, shift = true)),
    KeybindingOption.QUICK_PULL to listOf(Keybinding(key = Key.L, control = true, alt = true, shift = true)),
    KeybindingOption.QUICK_PUSH to listOf(Keybinding(key = Key.P, control = true, alt = true, shift = true)),
    KeybindingOption.TAG_CREATE to listOf(Keybinding(key = Key.T, control = true, shift = true)),
    KeybindingOption.CLONE to listOf(Keybinding(key = Key.N, control = true)),
    KeybindingOption.INIT_REPOSITORY to listOf(Keybinding(key = Key.N, control = true, shift = true)),
    KeybindingOption.SHOW_CHANGES to listOf(Keybinding(key = Key.One, control = true)),
    KeybindingOption.SHOW_ALL_COMMITS to listOf(Keybinding(key = Key.Two, control = true)),
    KeybindingOption.REVEAL_HEAD to listOf(Keybinding(key = Key.Zero, control = true)),
    KeybindingOption.ZOOM_IN to listOf(Keybinding(key = Key.Equals, control = true)),
    KeybindingOption.ZOOM_OUT to listOf(Keybinding(key = Key.Minus, control = true)),
    KeybindingOption.COMMIT_SEARCH to listOf(Keybinding(key = Key.F, control = true)),
    KeybindingOption.COMMIT_AND_PUSH to listOf(Keybinding(key = Key.Enter, control = true, shift = true)),
    KeybindingOption.STAGE_TOGGLE_SELECTED to listOf(Keybinding(key = Key.S, control = true, shift = true)),
    KeybindingOption.STAGE_TOGGLE_ALL to listOf(Keybinding(key = Key.S, control = true, alt = true, shift = true)),
    KeybindingOption.DISCARD_SELECTED to listOf(
        Keybinding(key = Key.D, control = true, shift = true),
        Keybinding(key = Key.Backspace),
    ),
    KeybindingOption.OPEN_IN_FILE_MANAGER to listOf(Keybinding(key = Key.O, control = true, alt = true)),
    KeybindingOption.OPEN_IN_TERMINAL to listOf(Keybinding(key = Key.T, control = true, alt = true)),
    KeybindingOption.FILTER_ACTIVE_BRANCH to listOf(Keybinding(key = Key.A, control = true, shift = true)),
)

private fun linuxKeybindings(): Map<KeybindingOption, List<Keybinding>> = baseKeybindings()
private fun windowsKeybindings(): Map<KeybindingOption, List<Keybinding>> = baseKeybindings()

private fun macKeybindings(): Map<KeybindingOption, List<Keybinding>> {
    val macBindings = baseKeybindings().toMutableMap()

    val keysToReplaceControlWithCommand = listOf(
        KeybindingOption.TEXT_ACCEPT,
        KeybindingOption.REFRESH,
        KeybindingOption.PULL,
        KeybindingOption.PUSH,
        KeybindingOption.BRANCH_CREATE,
        KeybindingOption.STASH,
        KeybindingOption.STASH_POP,
        KeybindingOption.OPEN_REPOSITORY,
        KeybindingOption.OPEN_NEW_TAB,
        KeybindingOption.CLOSE_CURRENT_TAB,
        KeybindingOption.SETTINGS,
        KeybindingOption.QUICK_LAUNCH,
        KeybindingOption.FETCH,
        KeybindingOption.QUICK_FETCH,
        KeybindingOption.QUICK_PULL,
        KeybindingOption.QUICK_PUSH,
        KeybindingOption.TAG_CREATE,
        KeybindingOption.CLONE,
        KeybindingOption.INIT_REPOSITORY,
        KeybindingOption.SHOW_CHANGES,
        KeybindingOption.SHOW_ALL_COMMITS,
        KeybindingOption.REVEAL_HEAD,
        KeybindingOption.ZOOM_IN,
        KeybindingOption.ZOOM_OUT,
        KeybindingOption.COMMIT_SEARCH,
        KeybindingOption.COMMIT_AND_PUSH,
        KeybindingOption.STAGE_TOGGLE_SELECTED,
        KeybindingOption.STAGE_TOGGLE_ALL,
        KeybindingOption.DISCARD_SELECTED,
        KeybindingOption.OPEN_IN_FILE_MANAGER,
        KeybindingOption.OPEN_IN_TERMINAL,
        KeybindingOption.FILTER_ACTIVE_BRANCH,
    )

    for (key in keysToReplaceControlWithCommand) {
        val originalKeybindings = macBindings[key] ?: emptyList()
        val newKeybindings = originalKeybindings.map {
            it.copy(meta = it.control, control = false)
        }

        macBindings[key] = newKeybindings
    }

    val bindingsToReplace = listOf(
        KeybindingOption.CHANGE_CURRENT_TAB_LEFT to listOf(
            Keybinding(key = Key.Tab, control = true, shift = true),
        ),
        KeybindingOption.CHANGE_CURRENT_TAB_RIGHT to listOf(
            Keybinding(key = Key.Tab, control = true),
        )
    )

    for (key in bindingsToReplace) {
        macBindings[key.first] = key.second
    }

    return macBindings
}

val keybindings by lazy {
    return@lazy when (currentOs) {
        OS.LINUX -> linuxKeybindings()
        OS.WINDOWS -> windowsKeybindings()
        OS.MAC -> macKeybindings()
        OS.UNKNOWN -> baseKeybindings()
    }
}

fun KeyEvent.matchesBinding(keybindingOption: KeybindingOption): Boolean {
    val keybindings = keybindings

    val matchingKeybindingsList = keybindings[keybindingOption] ?: return false

    return matchingKeybindingsList.any { keybinding ->
        keybinding.alt == this.isAltPressed &&
                keybinding.control == this.isCtrlPressed &&
                keybinding.meta == this.isMetaPressed &&
                keybinding.shift == this.isShiftPressed &&
                keybinding.key == this.key
    } && this.type == KeyEventType.KeyDown
}

val KeybindingOption.keyBinding
    get() = keybindings[this]?.firstOrNull()