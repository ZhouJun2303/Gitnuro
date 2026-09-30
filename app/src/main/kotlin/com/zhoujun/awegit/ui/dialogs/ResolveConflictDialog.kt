package com.zhoujun.awegit.ui.dialogs

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.conflicts.ConflictBlock
import com.zhoujun.awegit.domain.conflicts.ConflictParser
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.usecases.ResolveConflictUseCase
import com.zhoujun.awegit.theme.forkBorder
import com.zhoujun.awegit.theme.onBackgroundSecondary
import com.zhoujun.awegit.ui.components.fork.ForkButton
import com.zhoujun.awegit.ui.dialogs.base.MaterialDialog
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch

class ResolveConflictViewModel @AssistedInject constructor(
    private val resolveConflictUseCase: ResolveConflictUseCase,
    @Assisted val path: String,
) : TabViewModel() {
    @AssistedFactory
    interface Factory {
        fun create(path: String): ResolveConflictViewModel
    }

    val mine = MutableStateFlow("")
    val theirs = MutableStateFlow("")
    val result = MutableStateFlow("")
    val error = MutableStateFlow<String?>(null)

    fun load() {
        viewModelScope.launch {
            when (val loaded = resolveConflictUseCase.read(path)) {
                is Either.Ok -> {
                    val working = loaded.value.workingTree
                    val blocks = ConflictParser.parse(working)
                    val parsed = blocks.any { it is ConflictBlock.Conflict }
                    mine.value = if (parsed) columnText(blocks) { it.mine } else loaded.value.mine
                    theirs.value = if (parsed) columnText(blocks) { it.theirs } else loaded.value.theirs
                    result.value = if (parsed) ConflictParser.render(blocks) else working
                    error.value = null
                }
                is Either.Err -> error.value = loaded.error.toString()
            }
        }
    }

    fun useMine() = apply { resolveConflictUseCase.useMine(path) }

    fun useTheirs() = apply { resolveConflictUseCase.useTheirs(path) }

    fun markResolved(onDone: () -> Unit) {
        viewModelScope.launch {
            resolveConflictUseCase.markResolved(path)
            onDone()
        }
    }

    private fun apply(block: suspend () -> Unit) {
        viewModelScope.launch {
            block()
            load()
        }
    }

    private fun columnText(blocks: List<ConflictBlock>, pick: (ConflictBlock.Conflict) -> List<String>): String {
        return blocks.joinToString("\n") { block ->
            when (block) {
                is ConflictBlock.Common -> block.lines.joinToString("\n")
                is ConflictBlock.Conflict -> pick(block).joinToString("\n")
            }
        }
    }
}

@Composable
fun ResolveConflictDialog(
    viewModel: ResolveConflictViewModel,
    onDismiss: () -> Unit,
) {
    val mine by viewModel.mine.collectAsState()
    val theirs by viewModel.theirs.collectAsState()
    val result by viewModel.result.collectAsState()
    val error by viewModel.error.collectAsState()

    LaunchedEffect(viewModel) { viewModel.load() }

    MaterialDialog(onCloseRequested = onDismiss, paddingHorizontal = 16.dp, paddingVertical = 16.dp) {
        Column(Modifier.fillMaxWidth()) {
            Text(viewModel.path, fontSize = 14.sp, modifier = Modifier.padding(bottom = 8.dp))
            Row(Modifier.padding(bottom = 8.dp)) {
                ForkButton("Use Mine", viewModel::useMine, primary = true)
                ForkButton("Use Theirs", viewModel::useTheirs, modifier = Modifier.padding(start = 8.dp), primary = true)
                ForkButton(
                    "Mark as Resolved",
                    { viewModel.markResolved(onDismiss) },
                    modifier = Modifier.padding(start = 8.dp),
                )
                ForkButton("Close", onDismiss, modifier = Modifier.padding(start = 8.dp))
            }
            if (error != null) {
                Text(error.orEmpty(), color = MaterialTheme.colors.error, fontSize = 12.sp)
            }
            Row(Modifier.fillMaxWidth().height(360.dp)) {
                ConflictColumn("Mine", mine, Modifier.weight(1f))
                ConflictColumn("Theirs", theirs, Modifier.weight(1f).padding(horizontal = 8.dp))
                ConflictColumn("Result", result, Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun ConflictColumn(title: String, text: String, modifier: Modifier) {
    Column(modifier.fillMaxWidth()) {
        Text(title, fontSize = 11.sp, color = MaterialTheme.colors.onBackgroundSecondary)
        Text(
            text.ifEmpty { " " },
            fontSize = 12.sp,
            modifier = Modifier
                .padding(top = 4.dp)
                .fillMaxWidth()
                .height(330.dp)
                .border(1.dp, MaterialTheme.colors.forkBorder)
                .verticalScroll(rememberScrollState())
                .padding(6.dp),
        )
    }
}
