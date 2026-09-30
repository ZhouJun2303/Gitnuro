package com.zhoujun.awegit.domain.repositories

import com.zhoujun.awegit.domain.models.NotificationData
import kotlinx.coroutines.flow.StateFlow

interface NotificationsRepository {
    val notifications: StateFlow<List<NotificationData>>

    suspend fun emitNotification(notificationData: NotificationData)
}