package com.zhoujun.awegit.ui.components.fork

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.DropdownMenu
import androidx.compose.material.DropdownMenuItem
import androidx.compose.material.Icon
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.app.generated.resources.Res
import com.zhoujun.awegit.app.generated.resources.done
import com.zhoujun.awegit.app.generated.resources.expand_more
import com.zhoujun.awegit.theme.ForkDimens
import com.zhoujun.awegit.theme.backgroundSelected
import com.zhoujun.awegit.theme.forkBorder
import com.zhoujun.awegit.theme.onBackgroundSecondary
import org.jetbrains.compose.resources.DrawableResource
import org.jetbrains.compose.resources.painterResource

@Composable
fun ForkButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    primary: Boolean = false,
    enabled: Boolean = true,
) {
    val shape = RoundedCornerShape(3.dp)
    val border = if (primary && enabled) MaterialTheme.colors.primary else MaterialTheme.colors.forkBorder
    Box(
        modifier = modifier
            .height(ForkDimens.ButtonHeight)
            .defaultMinSize(minWidth = ForkDimens.ButtonMinWidth)
            .clip(shape)
            .border(1.dp, border, shape)
            .background(MaterialTheme.colors.background)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text,
            fontSize = 12.sp,
            color = if (enabled) MaterialTheme.colors.onBackground else MaterialTheme.colors.onBackgroundSecondary,
        )
    }
}

@Composable
fun ForkFormRow(
    label: String,
    labelWidth: Dp = ForkDimens.LabelWidth,
    content: @Composable RowScope.() -> Unit,
) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = ForkDimens.FormRowSpacing / 2),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            label,
            modifier = Modifier.width(labelWidth),
            textAlign = TextAlign.End,
            fontSize = 12.sp,
            color = MaterialTheme.colors.onBackground,
        )
        Spacer(Modifier.width(ForkDimens.LabelGap))
        content()
    }
}

@Composable
fun ForkCheckbox(checked: Boolean) {
    val shape = RoundedCornerShape(3.dp)
    Box(
        Modifier
            .size(14.dp)
            .clip(shape)
            .background(if (checked) MaterialTheme.colors.primary else MaterialTheme.colors.background)
            .border(1.dp, if (checked) MaterialTheme.colors.primary else MaterialTheme.colors.forkBorder, shape),
        contentAlignment = Alignment.Center,
    ) {
        if (checked) {
            Icon(
                painterResource(Res.drawable.done),
                contentDescription = null,
                modifier = Modifier.size(12.dp),
                tint = Color.White,
            )
        }
    }
}

@Composable
fun ForkCheckboxRow(
    text: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    labelWidth: Dp = ForkDimens.LabelWidth,
) {
    Row(
        Modifier.fillMaxWidth().height(ForkDimens.CheckboxRowHeight),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Spacer(Modifier.width(labelWidth + ForkDimens.LabelGap))
        Row(
            Modifier.clickable { onCheckedChange(!checked) },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ForkCheckbox(checked)
            Spacer(Modifier.width(6.dp))
            Text(text, fontSize = 12.sp, color = MaterialTheme.colors.onBackground)
        }
    }
}

@Composable
fun <T> ForkDropdown(
    items: List<T>,
    selected: T?,
    itemLabel: (T) -> String,
    onSelected: (T) -> Unit,
    modifier: Modifier = Modifier,
    leadingIcon: DrawableResource? = null,
) {
    var expanded by remember { mutableStateOf(false) }
    val shape = RoundedCornerShape(3.dp)
    Box(modifier) {
        Row(
            Modifier
                .fillMaxWidth()
                .height(ForkDimens.ControlHeight)
                .clip(shape)
                .border(1.dp, MaterialTheme.colors.forkBorder, shape)
                .background(MaterialTheme.colors.background)
                .clickable { expanded = true }
                .padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (leadingIcon != null) {
                Icon(
                    painterResource(leadingIcon),
                    contentDescription = null,
                    modifier = Modifier.size(14.dp),
                    tint = MaterialTheme.colors.onBackground,
                )
                Spacer(Modifier.width(6.dp))
            }
            Text(
                selected?.let(itemLabel).orEmpty(),
                fontSize = 12.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Icon(
                painterResource(Res.drawable.expand_more),
                contentDescription = null,
                modifier = Modifier.size(16.dp),
                tint = MaterialTheme.colors.onBackgroundSecondary,
            )
        }
        var filter by remember { mutableStateOf("") }
        val shown = if (items.size > 12 && filter.isNotBlank()) {
            items.filter { itemLabel(it).contains(filter, ignoreCase = true) }
        } else {
            items
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false; filter = "" }) {
            if (items.size > 12) {
                Box(Modifier.padding(horizontal = 8.dp, vertical = 4.dp)) {
                    BasicTextField(
                        value = filter,
                        onValueChange = { filter = it },
                        singleLine = true,
                        textStyle = TextStyle(color = MaterialTheme.colors.onBackground, fontSize = 12.sp),
                        modifier = Modifier.fillMaxWidth().height(ForkDimens.ControlHeight),
                    )
                }
            }
            shown.forEach { item ->
                DropdownMenuItem(onClick = {
                    expanded = false
                    filter = ""
                    onSelected(item)
                }) {
                    Text(itemLabel(item), fontSize = 12.sp)
                }
            }
        }
    }
}

@Composable
fun ForkTextField(
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
    singleLine: Boolean = true,
    minHeight: Dp = ForkDimens.ControlHeight,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    enabled: Boolean = true,
    placeholder: String? = null,
) {
    val shape = RoundedCornerShape(3.dp)
    val textColor = if (enabled) MaterialTheme.colors.onBackground else MaterialTheme.colors.onBackgroundSecondary
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = modifier
            .defaultMinSize(minHeight = minHeight)
            .clip(shape)
            .border(1.dp, MaterialTheme.colors.forkBorder, shape)
            .background(MaterialTheme.colors.background)
            .padding(horizontal = 8.dp, vertical = 4.dp),
        enabled = enabled,
        singleLine = singleLine,
        textStyle = TextStyle(color = textColor, fontSize = 12.sp),
        visualTransformation = visualTransformation,
        cursorBrush = SolidColor(MaterialTheme.colors.primary),
        decorationBox = { innerTextField ->
            Box(
                modifier = Modifier.fillMaxWidth(),
                contentAlignment = if (singleLine) Alignment.CenterStart else Alignment.TopStart,
            ) {
                if (value.text.isEmpty() && placeholder != null) {
                    Text(placeholder, fontSize = 12.sp, color = MaterialTheme.colors.onBackgroundSecondary)
                }
                innerTextField()
            }
        },
    )
}

@Composable
fun ForkTextField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    singleLine: Boolean = true,
    minHeight: Dp = ForkDimens.ControlHeight,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    enabled: Boolean = true,
    placeholder: String? = null,
) {
    ForkTextField(
        value = TextFieldValue(value),
        onValueChange = { onValueChange(it.text) },
        modifier = modifier,
        singleLine = singleLine,
        minHeight = minHeight,
        visualTransformation = visualTransformation,
        enabled = enabled,
        placeholder = placeholder,
    )
}

@Composable
fun ForkSegmentedButtons(
    options: List<String>,
    selected: String,
    onSelected: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(modifier.height(ForkDimens.ControlHeight).border(1.dp, MaterialTheme.colors.forkBorder, RoundedCornerShape(3.dp))) {
        for (option in options) {
            val active = option == selected
            Box(
                Modifier
                    .weight(1f)
                    .height(ForkDimens.ControlHeight)
                    .background(if (active) MaterialTheme.colors.backgroundSelected else MaterialTheme.colors.background)
                    .clickable { onSelected(option) },
                contentAlignment = Alignment.Center,
            ) {
                Text(option, fontSize = 12.sp, maxLines = 1)
            }
        }
    }
}

@Composable
fun ForkToolbarButton(
    icon: org.jetbrains.compose.resources.DrawableResource,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    onDropdown: (() -> Unit)? = null,
) {
    Column(
        modifier.width(56.dp).clickable(onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(painterResource(icon), contentDescription = label, modifier = Modifier.size(20.dp))
        Text(label, fontSize = 11.sp, maxLines = 1)
        if (onDropdown != null) {
            Icon(
                painterResource(Res.drawable.expand_more),
                contentDescription = null,
                modifier = Modifier.size(12.dp).clickable(onClick = onDropdown),
            )
        }
    }
}

@Composable
fun ForkBadge(
    text: String,
    color: Color,
    modifier: Modifier = Modifier,
    icon: org.jetbrains.compose.resources.DrawableResource? = null,
) {
    Row(
        modifier
            .height(16.dp)
            .clip(RoundedCornerShape(3.dp))
            .background(color)
            .padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Icon(painterResource(icon), contentDescription = null, modifier = Modifier.size(10.dp))
            Spacer(Modifier.width(2.dp))
        }
        Text(text, fontSize = 11.sp, maxLines = 1)
    }
}

@Composable
fun ForkSplitPane(
    ratio: Float,
    onRatioChange: (Float) -> Unit,
    modifier: Modifier = Modifier,
    horizontal: Boolean = true,
    first: @Composable () -> Unit,
    second: @Composable () -> Unit,
) {
    BoxWithConstraints(modifier) {
        val total = if (horizontal) maxHeight else maxWidth
        val totalPx = with(LocalDensity.current) { total.toPx() }.coerceAtLeast(1f)
        if (horizontal) {
            Column(Modifier.fillMaxWidth()) {
                Box(Modifier.fillMaxWidth().height(total * ratio.coerceIn(0.15f, 0.85f))) { first() }
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(4.dp)
                        .background(MaterialTheme.colors.forkBorder)
                        .pointerInput(totalPx) {
                            detectDragGestures { change, drag ->
                                change.consume()
                                onRatioChange((ratio + drag.y / totalPx).coerceIn(0.15f, 0.85f))
                            }
                        },
                )
                Box(Modifier.fillMaxWidth().weight(1f)) { second() }
            }
        } else {
            Row(Modifier.fillMaxWidth()) {
                Box(Modifier.fillMaxHeight().width(total * ratio.coerceIn(0.15f, 0.85f))) { first() }
                Box(
                    Modifier
                        .fillMaxHeight()
                        .width(4.dp)
                        .background(MaterialTheme.colors.forkBorder)
                        .pointerInput(totalPx) {
                            detectDragGestures { change, drag ->
                                change.consume()
                                onRatioChange((ratio + drag.x / totalPx).coerceIn(0.15f, 0.85f))
                            }
                        },
                )
                Box(Modifier.fillMaxHeight().weight(1f)) { second() }
            }
        }
    }
}
