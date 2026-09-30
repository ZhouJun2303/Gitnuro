package com.zhoujun.awegit.compose

import androidx.compose.runtime.Composable
import androidx.compose.runtime.currentCompositeKeyHashCode
import androidx.compose.runtime.remember
import com.zhoujun.awegit.LocalTab

@Composable
fun <T : Any> rememberInTab(
    id: String,
    vararg inputs: Any?,
    init: () -> T,
): T {
    val tab = LocalTab.current
    val inputs = inputs.toList()

    return remember(id, inputs) {
        if (tab.savedStates.contains(id)) {
            val pair = tab.savedStates[id]
            if (pair?.first == inputs) {
                return@remember pair.second as T
            }
        }

        val newValue = init()
        tab.savedStates[id] = Pair(inputs, newValue)
        newValue
    }
}