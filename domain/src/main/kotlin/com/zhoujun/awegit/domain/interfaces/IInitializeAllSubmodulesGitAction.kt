package com.zhoujun.awegit.domain.interfaces

import org.eclipse.jgit.api.Git

interface IInitializeAllSubmodulesGitAction {
    suspend operator fun invoke(git: Git): Unit
}