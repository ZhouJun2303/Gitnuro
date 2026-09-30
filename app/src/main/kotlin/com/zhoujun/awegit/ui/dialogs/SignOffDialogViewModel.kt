package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.models.SignOffConfig
import com.zhoujun.awegit.domain.usecases.LoadSignOffConfigUseCase
import com.zhoujun.awegit.domain.usecases.SaveSignOffConfigUseCase
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

class SignOffDialogViewModel @Inject constructor(
    private val loadSignOffConfigUseCase: LoadSignOffConfigUseCase,
    private val saveSignOffConfigUseCase: SaveSignOffConfigUseCase,
) : TabViewModel() {
    private val _state = MutableStateFlow<SignOffState>(SignOffState.Loading)
    val state = _state.asStateFlow()

    fun loadSignOffFormat() {
        viewModelScope.launch {
            val signOffConfig = loadSignOffConfigUseCase()

            if (signOffConfig is Either.Ok) {
                _state.value = SignOffState.Loaded(signOffConfig.value)
            }
        }
    }

    fun saveSignOffFormat(newIsEnabled: Boolean, newFormat: String) {
        viewModelScope.launch {
            val hiddenRefs = (loadSignOffConfigUseCase() as? Either.Ok)?.value?.hiddenRefs.orEmpty()
            saveSignOffConfigUseCase(SignOffConfig(newIsEnabled, newFormat, hiddenRefs))
        }
    }
}

sealed interface SignOffState {
    object Loading : SignOffState
    data class Loaded(val signOffConfig: SignOffConfig) : SignOffState
}