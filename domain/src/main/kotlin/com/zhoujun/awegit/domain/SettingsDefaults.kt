package com.zhoujun.awegit.domain

import com.zhoujun.awegit.domain.models.AvatarProviderType
import com.zhoujun.awegit.domain.models.DateTimeFormat

object SettingsDefaults {
    val defaultAvatarProviderType = AvatarProviderType.Gravatar
    val defaultDateTimeFormat = DateTimeFormat(
        useSystemDefault = true,
        customFormat = "dd MMM yyyy",
        is24hours = true,
        useRelativeDate = true,
    )
}