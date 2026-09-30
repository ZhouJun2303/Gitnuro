package com.zhoujun.awegit.data.git.submodules

import com.zhoujun.awegit.domain.interfaces.IInitializeAllSubmodulesGitAction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.eclipse.jgit.api.Git
import javax.inject.Inject

class InitializeAllSubmodulesGitAction @Inject constructor() : IInitializeAllSubmodulesGitAction {
    override suspend operator fun invoke(git: Git): Unit = withContext(Dispatchers.IO) {
        git.submoduleInit()
            .call()
    }
}