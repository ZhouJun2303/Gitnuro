package com.zhoujun.awegit.domain.interfaces

import org.eclipse.jgit.api.Git

interface IGetSpecificCommitMessageGitAction {
    suspend operator fun invoke(git: Git, commitId: String): String
}