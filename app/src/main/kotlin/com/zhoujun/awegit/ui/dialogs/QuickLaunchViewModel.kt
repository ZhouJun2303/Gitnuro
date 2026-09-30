package com.zhoujun.awegit.ui.dialogs

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.collectLatestInViewModel
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.WorkspacesState
import com.zhoujun.awegit.domain.repositories.RepositoryDataRepository
import com.zhoujun.awegit.domain.repositories.dataOrNull
import com.zhoujun.awegit.domain.services.WorkspacesService
import com.zhoujun.awegit.domain.usecases.CheckoutBranchUseCase
import com.zhoujun.awegit.extensions.stateIn
import com.zhoujun.awegit.ui.AppViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import javax.inject.Inject

enum class QuickLaunchGroup { Repositories, Branches, Commands }

sealed interface QuickLaunchAction {
    data class OpenRepository(val path: String, val workspaceId: String) : QuickLaunchAction
    data class Checkout(val branch: Branch) : QuickLaunchAction
    data class Command(val id: String, val argument: String = "") : QuickLaunchAction
}

data class QuickLaunchItem(
    val group: QuickLaunchGroup,
    val title: String,
    val subtitle: String,
    val action: QuickLaunchAction,
)

class QuickLaunchViewModel @Inject constructor(
    private val workspacesService: WorkspacesService,
    private val repositoryDataRepository: RepositoryDataRepository,
    private val checkoutBranchUseCase: CheckoutBranchUseCase,
    private val appViewModel: AppViewModel,
) : TabViewModel() {
    val query = MutableStateFlow("")
    private val branches = MutableStateFlow<List<Branch>>(emptyList())

    val results = combine(query, branches, workspacesService.state, ::buildItems).stateIn(emptyList())

    init {
        repositoryDataRepository.localBranches.collectLatestInViewModel {
            branches.value = it.dataOrNull().orEmpty()
        }
    }

    fun onQueryChange(value: String) {
        query.value = value
    }

    fun openRepository(path: String, workspaceId: String) {
        appViewModel.openRepositoryInWorkspace(path, workspaceId)
    }

    fun checkout(branch: Branch) {
        checkoutBranchUseCase(branch)
    }

    private fun buildItems(text: String, branchList: List<Branch>, workspaces: WorkspacesState): List<QuickLaunchItem> {
        val queryText = text.trim()
        val repositories = workspaces.workspaces.flatMap { workspace ->
            workspace.repositories.map { path ->
                QuickLaunchItem(
                    group = QuickLaunchGroup.Repositories,
                    title = path.substringAfterLast('\\').substringAfterLast('/'),
                    subtitle = "${workspace.name}  $path",
                    action = QuickLaunchAction.OpenRepository(path, workspace.id),
                )
            }
        }
        val branchItems = branchList.map { branch ->
            QuickLaunchItem(
                group = QuickLaunchGroup.Branches,
                title = branch.simpleName,
                subtitle = "Checkout",
                action = QuickLaunchAction.Checkout(branch),
            )
        }
        val commands = commandItems(queryText)
        return (repositories + branchItems + commands)
            .mapNotNull { item ->
                val score = fuzzyScore(queryText, "${item.title} ${item.subtitle}") ?: return@mapNotNull null
                item to score
            }
            .sortedByDescending { it.second }
            .map { it.first }
            .distinctBy { "${it.group}:${it.title}:${it.subtitle}" }
    }

    private fun commandItems(text: String): List<QuickLaunchItem> {
        val historyArgument = if (text.startsWith("file history ", ignoreCase = true)) {
            text.substringAfter(" ").substringAfter(" ").trim()
        } else {
            ""
        }
        val historyTitle = if (historyArgument.isNotEmpty()) "File History $historyArgument" else "File History <path>"
        return listOf(
            command("Fetch", "Open fetch dialog", "fetch"),
            command("Pull", "Open pull dialog", "pull"),
            command("Push", "Open push dialog", "push"),
            command("Quick Fetch", "Fetch all remotes", "quick-fetch"),
            command("Quick Pull", "Pull immediately", "quick-pull"),
            command("Quick Push", "Push immediately", "quick-push"),
            command("Stash", "Stash local changes", "stash"),
            command("Create Branch", "Create a branch", "create-branch"),
            command("Create Tag", "Create a tag on the selected commit", "create-tag"),
            command("Open in Terminal", "Open the repository terminal", "terminal"),
            command("Open in File Manager", "Show the repository folder", "explorer"),
            command("Refresh", "Refresh the repository", "refresh"),
            command("Preferences", "Open preferences", "preferences"),
            command("Repository Settings", "Open repository settings", "repository-settings"),
            command("Git Flow Init", "Initialize Git Flow", "git-flow-init"),
            command("Git Flow Start", "Start a Git Flow branch", "git-flow-start"),
            command("Git Flow Finish", "Finish a Git Flow branch", "git-flow-finish"),
            command(historyTitle, "Open file history", "file-history", historyArgument),
        )
    }

    private fun command(title: String, subtitle: String, id: String, argument: String = "") = QuickLaunchItem(
        group = QuickLaunchGroup.Commands,
        title = title,
        subtitle = subtitle,
        action = QuickLaunchAction.Command(id, argument),
    )
}

fun fuzzyScore(query: String, text: String): Int? {
    if (query.isBlank()) return 1
    val q = query.lowercase()
    val t = text.lowercase()
    var queryIndex = 0
    var score = 0
    var previous = -2
    for (index in t.indices) {
        if (queryIndex < q.length && t[index] == q[queryIndex]) {
            score += 1
            val boundary = index == 0 || t[index - 1] == ' ' || t[index - 1] == '/' || t[index - 1] == '-'
            if (boundary) score += 8
            if (index == previous + 1) score += 4
            previous = index
            queryIndex++
        }
    }
    return if (queryIndex == q.length) score else null
}
