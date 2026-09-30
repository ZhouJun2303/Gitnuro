package com.zhoujun.awegit.domain.models

data class DateTimeFormat(
    val useSystemDefault: Boolean,
    val customFormat: String,
    val is24hours: Boolean,
    val useRelativeDate: Boolean,
)
