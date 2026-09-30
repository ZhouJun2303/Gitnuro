package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.collectLatestInViewModel
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.models.AuthorInfo
import com.zhoujun.awegit.domain.models.GitFlowConfig
import com.zhoujun.awegit.domain.models.Identity
import com.zhoujun.awegit.domain.models.Remote
import com.zhoujun.awegit.domain.models.RemoteInfo
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import com.zhoujun.awegit.domain.usecases.AddRemoteUseCase
import com.zhoujun.awegit.domain.usecases.DeleteRemoteInfoUseCase
import com.zhoujun.awegit.domain.usecases.LoadGitFlowConfigUseCase
import com.zhoujun.awegit.domain.usecases.SaveAuthorUseCase
import com.zhoujun.awegit.domain.usecases.SaveGitFlowConfigUseCase
import com.zhoujun.awegit.domain.usecases.UpdateRemoteUseCase
import java.io.File
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

class RepositorySettingsViewModel @Inject constructor(
    private val repositoryDataRepository: RepositoryDataRepository,
    private val saveAuthorUseCase: SaveAuthorUseCase,
    private val addRemoteUseCase: AddRemoteUseCase,
    private val updateRemoteUseCase: UpdateRemoteUseCase,
    private val deleteRemoteInfoUseCase: DeleteRemoteInfoUseCase,
    private val loadGitFlowConfigUseCase: LoadGitFlowConfigUseCase,
    private val saveGitFlowConfigUseCase: SaveGitFlowConfigUseCase,
) : TabViewModel() {
    val author: StateFlow<AuthorInfo?>
        field = MutableStateFlow(null)

    val remotes: StateFlow<List<RemoteInfo>>
        field = MutableStateFlow(emptyList())

    val gitFlow: StateFlow<GitFlowConfig>
        field = MutableStateFlow(GitFlowConfig())

    val ignoreText: StateFlow<String>
        field = MutableStateFlow("")

    init {
        repositoryDataRepository.author.collectLatestInViewModel {
            author.value = it.dataOrNull()
        }
        repositoryDataRepository.remotes.collectLatestInViewModel {
            remotes.value = it.dataOrNull().orEmpty()
        }
        viewModelScope.launch {
            val loaded = loadGitFlowConfigUseCase()
            if (loaded is Either.Ok) gitFlow.value = loaded.value
            val path = repositoryDataRepository.repositoryPath ?: return@launch
            val file = File(path, ".gitignore")
            ignoreText.value = if (file.isFile) file.readText() else ""
        }
    }

    fun saveAuthor(name: String, email: String) {
        val current = author.value ?: AuthorInfo(Identity(null, null), Identity(null, null))
        viewModelScope.launch {
            saveAuthorUseCase(current.copy(repositoryIdentity = Identity(name, email)))
        }
    }

    fun addRemote(name: String, url: String) {
        addRemoteUseCase(Remote(name.trim(), url.trim(), url.trim()))
    }

    fun updateRemote(remote: Remote) = updateRemoteUseCase(remote)

    fun deleteRemote(remoteInfo: RemoteInfo) = deleteRemoteInfoUseCase(remoteInfo)

    fun onIgnoreChange(value: String) {
        ignoreText.value = value
    }

    fun saveIgnore() {
        val path = repositoryDataRepository.repositoryPath ?: return
        File(path, ".gitignore").writeText(ignoreText.value)
    }

    fun onGitFlowChange(config: GitFlowConfig) {
        gitFlow.value = config
    }

    fun saveGitFlow() = saveGitFlowConfigUseCase(gitFlow.value)
}
