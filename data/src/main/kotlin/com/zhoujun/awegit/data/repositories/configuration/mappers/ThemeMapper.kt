package com.zhoujun.awegit.data.repositories.configuration.mappers

import com.zhoujun.awegit.data.mappers.DataMapper
import com.zhoujun.awegit.domain.models.ui.Theme
import javax.inject.Inject

private const val DARK = "dark"
private const val LIGHT = "light"
private const val FORK_LIGHT = "fork_light"
private const val FORK_DARK = "fork_dark"
private const val CUSTOM = "custom"

class ThemeMapper @Inject constructor() : DataMapper<Theme?, String?> {
    override fun toData(value: Theme?): String? {
        return when (value) {
            Theme.Light -> LIGHT
            Theme.Dark -> DARK
            Theme.ForkLight -> FORK_LIGHT
            Theme.ForkDark -> FORK_DARK
            Theme.Custom -> CUSTOM
            null -> null
        }
    }


    override fun toDomain(value: String?): Theme? {
        return when (value) {
            LIGHT -> Theme.Light
            DARK -> Theme.Dark
            FORK_LIGHT -> Theme.ForkLight
            FORK_DARK -> Theme.ForkDark
            CUSTOM -> Theme.Custom
            null -> null
            else -> throw IllegalStateException("Unhandled theme $value")
        }
    }
}