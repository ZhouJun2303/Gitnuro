package com.zhoujun.awegit.extensions

import androidx.compose.runtime.Composable
import com.zhoujun.awegit.LocalDateTimeFormat
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.*
import java.util.concurrent.ConcurrentHashMap

private data class DateFormatterKey(
    val useSystemDefault: Boolean,
    val customFormat: String,
    val is24h: Boolean,
    val showTime: Boolean,
    val locale: String,
    val kind: String,
)

private object DateFormatterCache {
    private val cache = ConcurrentHashMap<DateFormatterKey, DateTimeFormatter>()

    fun get(key: DateFormatterKey, factory: () -> DateTimeFormatter): DateTimeFormatter =
        cache.getOrPut(key, factory)
}

@Composable
fun Long.toSmartSystemString(
    allowRelative: Boolean = true,
    useSystemDefaultFormat: Boolean? = null,
    showTime: Boolean = false,
): String {
    return Instant.ofEpochSecond(this).toSmartSystemString(
        allowRelative,
        useSystemDefaultFormat,
        showTime,
    )
}

@Composable
fun Instant.toSmartSystemString(
    allowRelative: Boolean = true,
    useSystemDefaultFormat: Boolean? = null,
    showTime: Boolean = false,
): String {
    val dateTimeFormat = LocalDateTimeFormat.current
    val useSystemDefault = useSystemDefaultFormat ?: dateTimeFormat.useSystemDefault

    val zoneId = ZoneId.systemDefault()
    val localDate = atZone(zoneId).toLocalDate()
    val currentTime = LocalDate.now(zoneId)
    val locale = Locale.Builder()
        .setLanguageTag(System.getProperty("user.language"))
        .build()

    val formattedDate = if (
        dateTimeFormat.useRelativeDate &&
        allowRelative &&
        localDate.isTodayOrYesterday(currentTime)
    ) {
        if (localDate.dayOfMonth == currentTime.dayOfMonth)
            "Today"
        else
            "Yesterday"
    } else {
        val formatter = DateFormatterCache.get(
            DateFormatterKey(useSystemDefault, dateTimeFormat.customFormat, dateTimeFormat.is24hours, false, locale.toLanguageTag(), "date"),
        ) {
            if (useSystemDefault) DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM)
            else DateTimeFormatter.ofPattern(dateTimeFormat.customFormat, locale)
        }

        formatter.format(localDate)
    }

    val formattedTime = if (showTime) {
        val localDateTime = atZone(zoneId).toLocalDateTime()

        val timeFormatter = DateFormatterCache.get(
            DateFormatterKey(useSystemDefault, dateTimeFormat.customFormat, dateTimeFormat.is24hours, true, locale.toLanguageTag(), "time"),
        ) {
            when {
                useSystemDefault -> DateTimeFormatter.ofLocalizedTime(FormatStyle.SHORT)
                dateTimeFormat.is24hours -> DateTimeFormatter.ofPattern("HH:mm")
                else -> DateTimeFormatter.ofPattern("hh:mm a")
            }
        }

        timeFormatter.format(localDateTime)
    } else {
        ""
    }

    return "${formattedDate.trim()} ${formattedTime.trim()}".trim()
}

private fun LocalDate.isTodayOrYesterday(
    currentTime: LocalDate,
): Boolean {
    return this.year == currentTime.year &&
            this.month == currentTime.month &&
            this.dayOfMonth in (currentTime.dayOfMonth - 1)..currentTime.dayOfMonth
}