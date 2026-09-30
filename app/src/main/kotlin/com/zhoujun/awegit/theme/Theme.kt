@file:Suppress("unused")

package com.zhoujun.awegit.theme

import androidx.compose.material.Colors
import androidx.compose.material.MaterialTheme
import androidx.compose.runtime.*
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.zhoujun.awegit.domain.models.ui.LinesHeightType
import com.zhoujun.awegit.domain.models.ui.Theme
import com.zhoujun.awegit.ui.dropdowns.DropDownOption
import kotlinx.coroutines.flow.MutableStateFlow

private val defaultAppTheme: ColorsScheme = darkTheme
private var appTheme: MutableStateFlow<ColorsScheme> = MutableStateFlow(defaultAppTheme)
internal val LocalLinesHeight = compositionLocalOf { spacedLineHeight }

class LinesHeight internal constructor(
    val fileHeight: Dp,
    val logCommitHeight: Dp,
    val sidePanelItemHeight: Dp,
)

val spacedLineHeight = LinesHeight(
    fileHeight = 28.dp,
    logCommitHeight = 30.dp,
    sidePanelItemHeight = 28.dp,
)

val compactLineHeight = LinesHeight(
    fileHeight = 22.dp,
    logCommitHeight = 24.dp,
    sidePanelItemHeight = 22.dp,
)

@Composable
fun AppTheme(
    selectedTheme: Theme = Theme.Dark,
    linesHeightType: LinesHeightType = LinesHeightType.COMPACT,
    customTheme: ColorsScheme? = null,
    content: @Composable () -> Unit,
) {
    val theme = when (selectedTheme) {
        Theme.Light -> lightTheme
        Theme.Dark -> darkTheme
        Theme.ForkLight -> forkLightTheme
        Theme.ForkDark -> forkDarkTheme
        Theme.Custom -> customTheme ?: defaultAppTheme
    }

    val lineHeight = when (linesHeightType) {
        LinesHeightType.SPACED -> spacedLineHeight
        LinesHeightType.COMPACT -> compactLineHeight
    }

    appTheme.value = theme

    val composeColors = theme.toComposeColors()
    val compositionValues = arrayOf(LocalLinesHeight provides lineHeight)

    CompositionLocalProvider(values = compositionValues) {
        MaterialTheme(
            colors = composeColors,
            content = content,
            typography = typography(composeColors),
        )
    }

}

val MaterialTheme.linesHeight: LinesHeight
    @Composable
    @ReadOnlyComposable
    get() = LocalLinesHeight.current


internal val theme: ColorsScheme
    @Composable
    get() = appTheme.collectAsState().value

val Colors.backgroundSelected: Color
    @Composable
    get() = theme.backgroundSelected

val Colors.onBackgroundSecondary: Color
    @Composable
    get() = theme.onBackgroundSecondary

val Colors.secondarySurface: Color
    @Composable
    get() = theme.secondarySurface

val Colors.tertiarySurface: Color
    @Composable
    get() = theme.tertiarySurface

val Colors.addFile: Color
    @Composable
    get() = theme.addFile

val Colors.deleteFile: Color
    @Composable
    get() = theme.deletedFile

val Colors.modifyFile: Color
    @Composable
    get() = theme.modifiedFile

val Colors.conflictFile: Color
    @Composable
    get() = theme.conflictingFile

val Colors.abortButton: Color
    @Composable
    get() = theme.error

val Colors.scrollbarNormal: Color
    @Composable
    get() = theme.normalScrollbar

val Colors.scrollbarHover: Color
    @Composable
    get() = theme.hoverScrollbar

val Colors.dialogOverlay: Color
    @Composable
    get() = theme.dialogOverlay

val Colors.border: Color
    @Composable
    get() = theme.border

val Colors.sidebarBackground: Color
    @Composable
    get() = theme.sidebarBackground

val Colors.toolbarBackground: Color
    @Composable
    get() = theme.toolbarBackground

val Colors.selectionUnfocused: Color
    @Composable
    get() = theme.selectionUnfocused

val Colors.badgeLocal: Color
    @Composable
    get() = theme.badgeLocal

val Colors.badgeRemote: Color
    @Composable
    get() = theme.badgeRemote

val Colors.badgeTag: Color
    @Composable
    get() = theme.badgeTag

val Colors.badgeStash: Color
    @Composable
    get() = theme.badgeStash

val Colors.graphLaneColors: List<Color>
    @Composable
    get() = theme.graphLaneColors

fun currentGraphLaneColors(): List<Color> = appTheme.value.graphLaneColors

val Colors.diffLineAdded: Color
    @Composable
    get() = theme.diffLineAdded


val Colors.diffContentAdded: Color
    @Composable
    get() = theme.diffContentAdded

val Colors.diffLineRemoved: Color
    @Composable
    get() = theme.diffLineRemoved

val Colors.diffContentRemoved: Color
    @Composable
    get() = theme.diffContentRemoved

val Colors.diffKeyword: Color
    @Composable
    get() = theme.diffKeyword

val Colors.diffAnnotation: Color
    @Composable
    get() = theme.diffAnnotation

val Colors.diffComment: Color
    @Composable
    get() = theme.diffComment

val Colors.isDark: Boolean
    get() = !this.isLight


// TODO Do not hardcode theme here and use proper string resource
val themeLists = listOf(
    DropDownOption(Theme.ForkLight, "Fork Light"),
    DropDownOption(Theme.ForkDark, "Fork Dark"),
    DropDownOption(Theme.Light, "Light"),
    DropDownOption(Theme.Dark, "Dark"),
    DropDownOption(Theme.Custom, "Custom"),
)