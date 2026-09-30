package com.zhoujun.awegit.viewmodels

import androidx.compose.runtime.Immutable
import com.zhoujun.awegit.LogsRepository
import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.common.flows.combine
import com.zhoujun.awegit.common.printError
import com.zhoujun.awegit.domain.errors.AiRequestError
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.models.AiSettings
import com.zhoujun.awegit.domain.models.AppConfig
import com.zhoujun.awegit.domain.models.AvatarProviderType
import com.zhoujun.awegit.domain.models.ProxyType
import com.zhoujun.awegit.domain.models.ui.LinesHeightType
import com.zhoujun.awegit.domain.models.ui.Theme
import com.zhoujun.awegit.domain.services.AppSettingsService
import com.zhoujun.awegit.domain.usecases.ListAiModelsUseCase
import com.zhoujun.awegit.extensions.stateIn
import com.zhoujun.awegit.system.OpenUrlInBrowserUseCase
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

private const val TAG = "SettingsViewModel"

class SettingsViewModel @Inject constructor(
    private val appSettingsService: AppSettingsService,
    private val logsRepository: LogsRepository,
    private val openUrlInBrowserUseCase: OpenUrlInBrowserUseCase,
    private val listAiModelsUseCase: ListAiModelsUseCase,
) : TabViewModel() {
    private val aiModelsState = MutableStateFlow(AiModelsState())

    val settingsViewState = settingsState()
        .stateIn(emptySettingsState())

    fun onAction(action: SettingsAction) {
        when (action) {
            is SettingsAction.SetConfig -> setAppConfiguration(action.configuration)
            SettingsAction.OpenLogsFolder -> openLogsFolderInFileExplorer()
            SettingsAction.RefreshAiModels -> viewModelScope.launch {
                aiModelsState.value = aiModelsState.value.copy(isLoading = true, error = null)
                aiModelsState.value = when (val r = listAiModelsUseCase()) {
                    is Either.Ok -> AiModelsState(models = r.value)
                    is Either.Err -> AiModelsState(error = (r.error as? AiRequestError)?.message ?: r.error.toString())
                }
            }
        }
    }

    private fun setAppConfiguration(appConfig: AppConfig) = viewModelScope.launch {
        appSettingsService.setConfiguration(appConfig)
    }

    fun openLogsFolderInFileExplorer() {
        try {
            openUrlInBrowserUseCase(logsRepository.logsDirectory.absolutePath)
        } catch (e: Exception) {
            printError(TAG, "Failed to open logs dir: ${e.message.orEmpty()}", e)
        }
    }

    private fun settingsState(): Flow<SettingsViewState> {
        return combine(
            appSettingsService.scaleUi,
            appSettingsService.theme,
            appSettingsService.customTheme,
            appSettingsService.linesHeightType,
            appSettingsService.dateFormatUseDefault,
            appSettingsService.dateFormatCustomFormat,
            appSettingsService.dateFormatIs24h,
            appSettingsService.dateFormatUseRelative,
            appSettingsService.avatarProvider,
            appSettingsService.swapStatusPanes,
            appSettingsService.pullWithRebase,
            appSettingsService.pushWithLease,
            appSettingsService.fastForwardMerge,
            appSettingsService.autoStashOnMerge,
            appSettingsService.cloneDefaultDirectory,
            appSettingsService.useProxy,
            appSettingsService.proxyUseAuth,
            appSettingsService.proxyType,
            appSettingsService.proxyHostName,
            appSettingsService.proxyPortNumber,
            appSettingsService.proxyHostUser,
            appSettingsService.proxyHostPassword,
            appSettingsService.verifySsl,
            appSettingsService.cacheCredentialsInMemory,
            appSettingsService.terminalPath,
            appSettingsService.aiSettings,
            aiModelsState,
        ) { scaleUi,
            theme,
            customTheme,
            linesHeightType,
            dateFormatUseDefault,
            dateFormatCustomFormat,
            dateFormatIs24h,
            dateFormatUseRelative,
            avatarProvider,
            swapStatusPanes,
            pullWithRebase,
            pushWithLease,
            fastForwardMerge,
            autoStashOnMerge,
            cloneDefaultDirectory,
            useProxy,
            proxyUseAuth,
            proxyType,
            proxyHostName,
            proxyPortNumber,
            proxyHostUser,
            proxyHostPassword,
            verifySsl,
            cacheCredentialsInMemory,
            terminalPath,
            aiSettings,
            aiModels ->

            SettingsViewState(
                scaleUi,
                theme,
                customTheme,
                linesHeightType,
                dateFormatUseDefault,
                dateFormatCustomFormat,
                dateFormatIs24h,
                dateFormatUseRelative,
                avatarProvider,
                swapStatusPanes,
                pullWithRebase,
                pushWithLease,
                fastForwardMerge,
                autoStashOnMerge,
                cloneDefaultDirectory,
                useProxy,
                proxyUseAuth,
                proxyType,
                proxyHostName,
                proxyPortNumber,
                proxyHostUser,
                proxyHostPassword,
                verifySsl,
                cacheCredentialsInMemory,
                terminalPath,
                aiSettings,
                aiModels,
            )
        }
    }

    private fun emptySettingsState(): SettingsViewState {
        return SettingsViewState(
            scaleUi = null,
            theme = Theme.Light,
            customTheme = "",
            linesHeightType = LinesHeightType.SPACED,
            dateFormatUseDefault = false,
            dateFormatCustomFormat = "",
            dateFormatIs24h = false,
            dateFormatUseRelative = false,
            avatarProvider = AvatarProviderType.Gravatar,
            swapStatusPanes = false,
            pullWithRebase = false,
            pushWithLease = false,
            fastForwardMerge = false,
            autoStashOnMerge = false,
            cloneDefaultDirectory = "",
            useProxy = false,
            proxyUseAuth = false,
            proxyType = ProxyType.HTTP,
            proxyHostName = "",
            proxyPortNumber = null,
            proxyHostUser = "",
            proxyHostPassword = "",
            verifySsl = false,
            cacheCredentialsInMemory = false,
            terminalPath = "",
            aiSettings = AiSettings(),
            aiModels = AiModelsState(),
        )
    }
}

@Immutable
data class SettingsViewState(
    val scaleUi: Float?,
    val theme: Theme,
    val customTheme: String?,
    val linesHeightType: LinesHeightType,
    val dateFormatUseDefault: Boolean,
    val dateFormatCustomFormat: String,
    val dateFormatIs24h: Boolean,
    val dateFormatUseRelative: Boolean,
    val avatarProvider: AvatarProviderType,
    val swapStatusPanes: Boolean,
    val pullWithRebase: Boolean,
    val pushWithLease: Boolean,
    val fastForwardMerge: Boolean,
    val autoStashOnMerge: Boolean,
    val cloneDefaultDirectory: String?,
    val useProxy: Boolean,
    val proxyUseAuth: Boolean,
    val proxyType: ProxyType,
    val proxyHostName: String?,
    val proxyPortNumber: Int?,
    val proxyHostUser: String?,
    val proxyHostPassword: String?,
    val verifySsl: Boolean,
    val cacheCredentialsInMemory: Boolean,
    val terminalPath: String?,
    val aiSettings: AiSettings,
    val aiModels: AiModelsState,
)

data class AiModelsState(
    val models: List<String> = emptyList(),
    val isLoading: Boolean = false,
    val error: String? = null,
)

sealed interface SettingsAction {
    data class SetConfig(val configuration: AppConfig) : SettingsAction
    data object OpenLogsFolder : SettingsAction
    data object RefreshAiModels : SettingsAction
}