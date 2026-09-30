package com.zhoujun.awegit.domain.repositories

import com.zhoujun.awegit.domain.models.ProxySettings

interface SystemProxyRepository {
    suspend fun setProxy(proxySettings: ProxySettings)
    suspend fun clearProxy()
}