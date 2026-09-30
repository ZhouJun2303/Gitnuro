package com.zhoujun.awegit.avatarproviders


class NoneAvatarProvider : AvatarProvider {
    override fun getAvatarUrl(hashedEmail: String): String? {
        return null
    }
}