package com.zhoujun.awegit.data.git.workspace

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.interfaces.IUnstageByDirectoryGitAction
import javax.inject.Inject

class UnstageByDirectoryGitAction @Inject constructor(
    private val jgit: JGit,
) : IUnstageByDirectoryGitAction {
    override suspend operator fun invoke(repositoryPath: String, dir: String): Either<Unit, GitError> =
        jgit.provide(repositoryPath) { git ->
            git
                .reset()
                .addPath(dir)
                .call()
        }
}
