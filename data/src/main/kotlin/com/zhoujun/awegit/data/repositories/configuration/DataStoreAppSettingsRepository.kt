package com.zhoujun.awegit.data.repositories.configuration

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.*
import com.zhoujun.awegit.common.AppDirectories
import com.zhoujun.awegit.common.OS
import com.zhoujun.awegit.common.currentOs
import com.zhoujun.awegit.data.UserSettingsDataStore
import com.zhoujun.awegit.data.repositories.configuration.mappers.AvatarProviderMapper
import com.zhoujun.awegit.data.repositories.configuration.mappers.LinesHeightMapper
import com.zhoujun.awegit.data.repositories.configuration.mappers.TextDiffViewTypeMapper
import com.zhoujun.awegit.data.repositories.configuration.mappers.ThemeMapper
import com.zhoujun.awegit.domain.models.AiSettings
import com.zhoujun.awegit.domain.models.AppConfig
import com.zhoujun.awegit.domain.models.ProxyType
import com.zhoujun.awegit.domain.models.ui.AppWindowPlacement
import com.zhoujun.awegit.domain.repositories.AppSettingsRepository
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import javax.inject.Inject
import java.util.prefs.Preferences as LegacyPreferences

private const val PREFERENCES_NAME = "AweGitConfig"
private const val LEGACY_PREFERENCES_NAME = "GitnuroConfig"

private const val PREF_LATEST_REPOSITORIES_TABS_OPENED = "latestRepositoriesTabsOpened"
private const val PREF_LATEST_REPOSITORY_TAB_SELECTED = "latestRepositoryTabSelected"
private const val PREF_LAST_OPENED_REPOSITORIES_PATH = "lastOpenedRepositoriesList"
private const val PREF_WINDOW_PLACEMENT = "windowsPlacement"
private const val PREF_FIRST_PANE_WIDTH = "firstPaneWidth"
private const val PREF_THIRD_PANE_WIDTH = "thirdPaneWidth"
private const val DEFAULT_FIRST_PANE_WIDTH = 220f
private const val DEFAULT_THIRD_PANE_WIDTH = 330f

private val scaleUiPreference get() = floatPreferencesKey("scale_ui")
private val themePreference get() = stringPreferencesKey("theme")
private val customThemePreference get() = stringPreferencesKey("custom_theme")
private val linesHeightPreference get() = stringPreferencesKey("lines_height")
private val swapStatusPanesPreference get() = booleanPreferencesKey("swap_status_panes")
private val diffDisplayFullFilePreference get() = booleanPreferencesKey("diff_display_full_file")
private val diffTextViewTypePreference get() = stringPreferencesKey("diff_text_view")
private val showChangesAsTreePreference get() = booleanPreferencesKey("show_changes_as_tree")

private val dateFormatUseDefaultPreference get() = booleanPreferencesKey("date_format_use_default")
private val dateFormatCustomFormatPreference get() = stringPreferencesKey("date_format_custom_format")
private val dateFormatIs24hPreference get() = booleanPreferencesKey("date_format_is_24h")
private val dateFormatUseRelativePreference get() = booleanPreferencesKey("date_format_use_relative")

private val avatarProviderPreference get() = stringPreferencesKey("avatar_provider")

private val fastForwardMergePreference get() = booleanPreferencesKey("fast_forward_merge")
private val autoStashOnMergePreference get() = booleanPreferencesKey("auto_stash_on_merge")
private val pullWithRebasePreference get() = booleanPreferencesKey("pull_with_rebase")
private val pullDialogRebasePreference get() = booleanPreferencesKey("pull_dialog_rebase")
private val pullDialogStashAndReapplyPreference get() = booleanPreferencesKey("pull_dialog_stash_and_reapply")
private val aiSettingsPreference get() = stringPreferencesKey("ai_settings")
private val aiJson = Json { ignoreUnknownKeys = true }
private val pushWithLeasePreference get() = booleanPreferencesKey("push_with_lease")
private val cloneDefaultDirectoryPreference get() = stringPreferencesKey("clone_default_directory")

private val useProxyPreference get() = booleanPreferencesKey("use_proxy")
private val proxyUseAuthPreference get() = booleanPreferencesKey("proxy_use_auth")
private val proxyProxyTypePreference get() = intPreferencesKey("proxy_type")
private val proxyHostNamePreference get() = stringPreferencesKey("proxy_host_name")
private val proxyPortNumberPreference get() = intPreferencesKey("proxy_port_number")
private val proxyHostUserPreference get() = stringPreferencesKey("proxy_host_user")
private val proxyHostPasswordPreference get() = stringPreferencesKey("proxy_host_password")
private val cacheCredentialsPreference get() = booleanPreferencesKey("cache_credentials_in_memory")
private val terminalPathPreference get() = stringPreferencesKey("terminal_path")

private val verifySslPreference get() = booleanPreferencesKey("verify_ssl")

operator fun <T> Flow<Preferences>.get(key: Preferences.Key<T>): Flow<T?> {
    return this.map { it[key] }
}

suspend fun <T> DataStore<Preferences>.setValue(key: Preferences.Key<T>, value: T?) {
    this.updateData {
        it.toMutablePreferences().also { preferences ->
            if (value == null) {
                preferences.remove(key)
            } else {
                preferences[key] = value
            }
        }
    }
}

class DataStoreAppSettingsRepository @Inject constructor(
    private val userSettingsDataStore: UserSettingsDataStore,
    private val themeMapper: ThemeMapper,
    private val linesHeightMapper: LinesHeightMapper,
    private val avatarProviderMapper: AvatarProviderMapper,
    private val textDiffViewTypeMapper: TextDiffViewTypeMapper,
) : AppSettingsRepository {
    private val preferences = userSettingsDataStore.preferences

    // UI
    override val scaleUi = preferences.data[scaleUiPreference]

    override val theme = preferences.data[themePreference].map { themeMapper.toDomain(it) }
    override val customTheme = preferences.data[customThemePreference]

    override val linesHeightType =
        preferences.data[linesHeightPreference].map { linesHeightMapper.toDomain(it) }

    override val dateFormatUseDefault get() = preferences.data[dateFormatUseDefaultPreference]
    override val dateFormatCustomFormat get() = preferences.data[dateFormatCustomFormatPreference]
    override val dateFormatIs24h get() = preferences.data[dateFormatIs24hPreference]
    override val dateFormatUseRelative get() = preferences.data[dateFormatUseRelativePreference]

    override val avatarProvider get() = preferences.data[avatarProviderPreference].map { avatarProviderMapper.toDomain(it) }
    override val swapStatusPanes get() = preferences.data[swapStatusPanesPreference]
    override val showChangesAsTree get() = preferences.data[showChangesAsTreePreference]
    override val diffDisplayFullFile get() = preferences.data[diffDisplayFullFilePreference]
    override val diffTextViewType get() = preferences.data[diffTextViewTypePreference].map { textDiffViewTypeMapper.toDomain(it) }

    // Git
    override val pullWithRebase get() = preferences.data[pullWithRebasePreference]
    override val pullDialogRebase get() = preferences.data[pullDialogRebasePreference]
    override val pullDialogStashAndReapply get() = preferences.data[pullDialogStashAndReapplyPreference]
    override val aiSettings = preferences.data[aiSettingsPreference].map { raw ->
        raw?.let { runCatching { aiJson.decodeFromString<AiSettings>(it) }.getOrNull() }
    }
    override val pushWithLease get() = preferences.data[pushWithLeasePreference]
    override val fastForwardMerge get() = preferences.data[fastForwardMergePreference]
    override val autoStashOnMerge get() = preferences.data[autoStashOnMergePreference]
    override val cloneDefaultDirectory get() = preferences.data[cloneDefaultDirectoryPreference]


    // Network
    override val useProxy get() = preferences.data[useProxyPreference]
    override val proxyUseAuth get() = preferences.data[proxyUseAuthPreference]
    override val proxyType get() = preferences.data[proxyProxyTypePreference].map { ProxyType.fromValue(it) }
    override val proxyHostName get() = preferences.data[proxyHostNamePreference]
    override val proxyPortNumber get() = preferences.data[proxyPortNumberPreference]
    override val proxyHostUser get() = preferences.data[proxyHostUserPreference]
    override val proxyHostPassword get() = preferences.data[proxyHostPasswordPreference]

    override val verifySsl get() = preferences.data[verifySslPreference]
    override val cacheCredentialsInMemory get() = preferences.data[cacheCredentialsPreference]

    // Tools
    override val terminalPath get() = preferences.data[terminalPathPreference]

    override suspend fun setConfiguration(appConfig: AppConfig) {
        preferences.apply {
            when (appConfig) {
                is AppConfig.AutoStashOnMerge -> setValue(autoStashOnMergePreference, appConfig.value)
                is AppConfig.CloneDefaultDirectory -> setValue(cloneDefaultDirectoryPreference, appConfig.value)
                is AppConfig.DateFormatCustomFormat -> setValue(dateFormatCustomFormatPreference, appConfig.value)
                is AppConfig.DateFormatIs24h -> setValue(dateFormatIs24hPreference, appConfig.value)
                is AppConfig.DateFormatUseDefault -> setValue(dateFormatUseDefaultPreference, appConfig.value)
                is AppConfig.DateFormatUseRelative -> setValue(dateFormatUseRelativePreference, appConfig.value)
                is AppConfig.FastForwardMerge -> setValue(fastForwardMergePreference, appConfig.value)
                is AppConfig.LinesHeight -> setValue(linesHeightPreference, linesHeightMapper.toData(appConfig.value))
                is AppConfig.UseProxy -> setValue(useProxyPreference, appConfig.value)
                is AppConfig.ProxyHostName -> setValue(proxyHostNamePreference, appConfig.value)
                is AppConfig.ProxyHostPassword -> setValue(proxyHostPasswordPreference, appConfig.value)
                is AppConfig.ProxyHostUser -> setValue(proxyHostUserPreference, appConfig.value)
                is AppConfig.ProxyPortNumber -> setValue(proxyPortNumberPreference, appConfig.value)
                is AppConfig.ProxyProxyType -> setValue(proxyProxyTypePreference, appConfig.value.value)
                is AppConfig.ProxyUseAuth -> setValue(proxyUseAuthPreference, appConfig.value)
                is AppConfig.PullWithRebase -> setValue(pullWithRebasePreference, appConfig.value)
                is AppConfig.PullDialogRebase -> setValue(pullDialogRebasePreference, appConfig.value)
                is AppConfig.PullDialogStashAndReapply -> setValue(pullDialogStashAndReapplyPreference, appConfig.value)
                is AppConfig.Ai -> setValue(aiSettingsPreference, aiJson.encodeToString(appConfig.value))
                is AppConfig.VerifySsl -> setValue(verifySslPreference, appConfig.value)
                is AppConfig.PushWithLease -> setValue(pushWithLeasePreference, appConfig.value)
                is AppConfig.ScaleUi -> setValue(scaleUiPreference, appConfig.value)
                is AppConfig.CacheCredentialsInMemory -> setValue(cacheCredentialsPreference, appConfig.value)
                is AppConfig.AvatarProvider -> setValue(
                    avatarProviderPreference,
                    avatarProviderMapper.toData(appConfig.value)
                )

                is AppConfig.Theme -> setValue(themePreference, themeMapper.toData(appConfig.value))
                is AppConfig.CustomTheme -> setValue(customThemePreference, appConfig.value)
                is AppConfig.SwapStatusPanes -> setValue(swapStatusPanesPreference, appConfig.value)
                is AppConfig.DiffDisplayFullFile -> setValue(diffDisplayFullFilePreference, appConfig.value)
                is AppConfig.DiffTextViewType -> setValue(diffTextViewTypePreference, textDiffViewTypeMapper.toData(appConfig.value))
                is AppConfig.ShowChangesAsTree -> setValue(showChangesAsTreePreference, appConfig.value)
                is AppConfig.TerminalPath -> setValue(terminalPathPreference, appConfig.value)
            }
        }
    }

    private val preferencesLegacy: LegacyPreferences = LegacyPreferences.userRoot().node(PREFERENCES_NAME)

    override var latestTabsOpened: String
        get() = preferencesLegacy.get(PREF_LATEST_REPOSITORIES_TABS_OPENED, "")
        set(value) {
            preferencesLegacy.put(PREF_LATEST_REPOSITORIES_TABS_OPENED, value)
        }

    override var latestRepositoryTabSelected: Int
        get() = preferencesLegacy.getInt(PREF_LATEST_REPOSITORY_TAB_SELECTED, -1)
        set(value) {
            preferencesLegacy.putInt(PREF_LATEST_REPOSITORY_TAB_SELECTED, value)
        }

    override var latestOpenedRepositoriesPath: String
        get() = preferencesLegacy.get(PREF_LAST_OPENED_REPOSITORIES_PATH, "")
        set(value) {
            preferencesLegacy.put(PREF_LAST_OPENED_REPOSITORIES_PATH, value)
        }

    override var firstPaneWidth: Float
        get() {
            return preferencesLegacy.getFloat(PREF_FIRST_PANE_WIDTH, DEFAULT_FIRST_PANE_WIDTH)
        }
        set(value) {
            preferencesLegacy.putFloat(PREF_FIRST_PANE_WIDTH, value)
        }

    override var thirdPaneWidth: Float
        get() {
            return preferencesLegacy.getFloat(PREF_THIRD_PANE_WIDTH, DEFAULT_THIRD_PANE_WIDTH)
        }
        set(value) {
            preferencesLegacy.putFloat(PREF_THIRD_PANE_WIDTH, value)
        }


    var windowPlacement: AppWindowPlacement
        get() {
            // FIXME Preference doing nothing here
            val placement = preferencesLegacy.getInt(PREF_WINDOW_PLACEMENT, 0)

            return AppWindowPlacement.MAXIMIZED
        }
        set(placement) {
            //preferences.putInt(PREF_WINDOW_PLACEMENT, placement.value)
        }
}

fun initPreferencesPath() {
    if (currentOs == OS.LINUX) System.setProperty("java.util.prefs.userRoot", AppDirectories.configDir().absolutePath)
}

fun getPreferencesPath(): String = AppDirectories.preferencesFile().absolutePath

/** 把 Gitnuro 的 Java Preferences 节点复制到 AweGit 节点，只执行一次 */
fun migrateLegacyPreferencesNode() {
    val root = LegacyPreferences.userRoot()
    if (root.nodeExists(PREFERENCES_NAME) || !root.nodeExists(LEGACY_PREFERENCES_NAME)) return
    val legacy = root.node(LEGACY_PREFERENCES_NAME)
    val current = root.node(PREFERENCES_NAME)
    for (key in legacy.keys()) legacy.get(key, null)?.let { current.put(key, it) }
    current.flush()
}