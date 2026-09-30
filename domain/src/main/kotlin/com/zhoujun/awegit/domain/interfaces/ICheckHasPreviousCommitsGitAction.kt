package com.zhoujun.awegit.domain.interfaces

import org.eclipse.jgit.api.Git

interface ICheckHasPreviousCommitsGitAction {
    suspend operator fun invoke(git: Git): Boolean
}