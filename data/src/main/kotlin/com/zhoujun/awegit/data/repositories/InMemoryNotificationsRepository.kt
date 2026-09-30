package com.zhoujun.awegit.data.repositories

import com.zhoujun.awegit.domain.models.NotificationData
import com.zhoujun.awegit.domain.repositories.NotificationsRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import javax.inject.Inject

class InMemoryNotificationsRepository @Inject constructor() : NotificationsRepository {
    private val _notifications = MutableStateFlow<List<NotificationData>>(emptyList())

    override val notifications: StateFlow<List<NotificationData>> = _notifications

    override suspend fun emitNotification(notificationData: NotificationData) {
        _notifications.value += notificationData
    }
}