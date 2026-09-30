package com.zhoujun.awegit.theme

import androidx.compose.material.Colors
import androidx.compose.material.Typography
import androidx.compose.runtime.Composable
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.platform.SystemFont
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.common.OS
import com.zhoujun.awegit.common.currentOs
import com.zhoujun.awegit.app.generated.resources.Inter_18pt_Bold
import com.zhoujun.awegit.app.generated.resources.Inter_18pt_BoldItalic
import com.zhoujun.awegit.app.generated.resources.Inter_18pt_Italic
import com.zhoujun.awegit.app.generated.resources.Inter_18pt_Medium
import com.zhoujun.awegit.app.generated.resources.Inter_18pt_MediumItalic
import com.zhoujun.awegit.app.generated.resources.Inter_18pt_Regular
import com.zhoujun.awegit.app.generated.resources.Inter_18pt_SemiBold
import com.zhoujun.awegit.app.generated.resources.Inter_18pt_SemiBoldItalic
import com.zhoujun.awegit.app.generated.resources.NotoSansMono_Bold
import com.zhoujun.awegit.app.generated.resources.NotoSansMono_Medium
import com.zhoujun.awegit.app.generated.resources.NotoSansMono_Regular
import com.zhoujun.awegit.app.generated.resources.NotoSansMono_SemiBold
import com.zhoujun.awegit.app.generated.resources.Res
import org.jetbrains.compose.resources.Font


const val LETTER_SPACING = 0.5

@Composable
fun typography(composeColors: Colors): Typography {
    val interFontFamily = FontFamily(
        Font(Res.font.Inter_18pt_Regular, FontWeight.Normal, FontStyle.Normal),
        Font(Res.font.Inter_18pt_Italic, FontWeight.Normal, FontStyle.Italic),
        Font(Res.font.Inter_18pt_Medium, FontWeight.Medium, FontStyle.Normal),
        Font(Res.font.Inter_18pt_MediumItalic, FontWeight.Medium, FontStyle.Italic),
        Font(Res.font.Inter_18pt_SemiBold, FontWeight.SemiBold, FontStyle.Normal),
        Font(Res.font.Inter_18pt_SemiBoldItalic, FontWeight.SemiBold, FontStyle.Italic),
        Font(Res.font.Inter_18pt_Bold, FontWeight.Bold, FontStyle.Normal),
        Font(Res.font.Inter_18pt_BoldItalic, FontWeight.Bold, FontStyle.Italic),
    )

    val uiFontFamily = when (currentOs) {
        OS.WINDOWS -> FontFamily(
            SystemFont("Segoe UI", FontWeight.Normal),
            SystemFont("Segoe UI", FontWeight.SemiBold),
            SystemFont("Segoe UI", FontWeight.Bold),
        )
        OS.MAC -> FontFamily.Default
        else -> interFontFamily
    }

    return Typography(
        defaultFontFamily = uiFontFamily,
        h1 = TextStyle(fontSize = 20.sp, color = composeColors.onBackground, letterSpacing = LETTER_SPACING.sp),
        h2 = TextStyle(fontSize = 16.sp, color = composeColors.onBackground, letterSpacing = LETTER_SPACING.sp),
        h3 = TextStyle(fontSize = 14.sp, color = composeColors.onBackground, letterSpacing = LETTER_SPACING.sp),
        h4 = TextStyle(fontSize = 13.sp, color = composeColors.onBackground, letterSpacing = LETTER_SPACING.sp),
        body1 = TextStyle(fontSize = 12.sp, color = composeColors.onBackground, letterSpacing = LETTER_SPACING.sp),
        body2 = TextStyle(fontSize = 12.sp, color = composeColors.onBackground, fontWeight = FontWeight.Normal),
        caption = TextStyle(fontSize = 11.sp, color = composeColors.onBackground, letterSpacing = LETTER_SPACING.sp),
        button = TextStyle(fontSize = 12.sp, color = composeColors.onBackground),
    )
}

@Composable
fun monoTypography(): FontFamily {
    val notoSansMonoFontFamily = FontFamily(
        Font(Res.font.NotoSansMono_Regular, FontWeight.Normal, FontStyle.Normal),
        Font(Res.font.NotoSansMono_Medium, FontWeight.Medium, FontStyle.Normal),
        Font(Res.font.NotoSansMono_SemiBold, FontWeight.SemiBold, FontStyle.Normal),
        Font(Res.font.NotoSansMono_Bold, FontWeight.Bold, FontStyle.Normal),
    )

    return notoSansMonoFontFamily
}