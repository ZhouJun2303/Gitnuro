package com.zhoujun.awegit.avatarproviders

interface AvatarProvider {
    fun getAvatarUrl(hashedEmail: String): String?
}