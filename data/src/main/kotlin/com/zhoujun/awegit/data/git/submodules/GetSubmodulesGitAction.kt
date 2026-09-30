package com.zhoujun.awegit.data.git.submodules

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.JGitSubmoduleMapper
import com.zhoujun.awegit.domain.interfaces.IGetSubmodulesGitAction
import javax.inject.Inject

class GetSubmodulesGitAction @Inject constructor(
    private val jgit: JGit,
    private val submoduleMapper: JGitSubmoduleMapper,
) : IGetSubmodulesGitAction {
    override suspend operator fun invoke(repositoryPath: String) = jgit.provide(repositoryPath) { git ->
        val submodules = git
            .submoduleStatus()
            .call()

        submodules
            .mapValues { submodule ->
                submoduleMapper.toDomain(submodule.value)
            }
            .toMap()
    }
}