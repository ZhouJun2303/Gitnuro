package com.zhoujun.awegit.data.repositories.configuration.mappers

import com.zhoujun.awegit.data.mappers.DataMapper
import com.zhoujun.awegit.domain.models.AvatarProviderType
import javax.inject.Inject

private const val NONE = "none"
private const val GRAVATAR = "gravatar"

class AvatarProviderMapper @Inject constructor(): DataMapper<AvatarProviderType?, String?>  {
    override fun toData(value: AvatarProviderType?): String? {
        return when (value) {
            AvatarProviderType.None -> NONE
            AvatarProviderType.Gravatar -> GRAVATAR
            null -> null
        }
    }

    override fun toDomain(value: String?): AvatarProviderType? {
        return when (value) {
            NONE -> AvatarProviderType.None
            GRAVATAR -> AvatarProviderType.Gravatar
            null -> null
            else -> throw IllegalStateException("Unhandled avatar provider $value")
        }
    }
}
