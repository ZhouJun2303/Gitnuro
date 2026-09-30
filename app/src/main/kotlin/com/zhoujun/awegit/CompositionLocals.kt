package com.zhoujun.awegit

import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.focus.FocusRequester
import com.zhoujun.awegit.avatarproviders.AvatarProvider
import com.zhoujun.awegit.avatarproviders.NoneAvatarProvider
import com.zhoujun.awegit.domain.SettingsDefaults
import com.zhoujun.awegit.viewmodels.RepositoryTabViewModel

val LocalTab =
    compositionLocalOf<RepositoryTabViewModel> { throw IllegalStateException("Tab information requested but not provided") }
val LocalTabFocusRequester = compositionLocalOf { FocusRequester() }
val LocalAvatarProvider = compositionLocalOf<AvatarProvider> { NoneAvatarProvider() }
val LocalDateTimeFormat = compositionLocalOf { SettingsDefaults.defaultDateTimeFormat }
