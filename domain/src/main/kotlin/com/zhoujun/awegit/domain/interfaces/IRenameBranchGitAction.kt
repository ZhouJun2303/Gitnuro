package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.models.Branch
import org.eclipse.jgit.api.Git
import org.eclipse.jgit.lib.Ref

interface IRenameBranchGitAction {
    suspend operator fun invoke(repositoryPath: String, oldName: String, newName: String): Either<Branch, GitError>
}