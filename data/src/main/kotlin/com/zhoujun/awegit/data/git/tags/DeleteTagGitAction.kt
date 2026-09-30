package com.zhoujun.awegit.data.git.tags

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.interfaces.IDeleteTagGitAction
import com.zhoujun.awegit.domain.models.Tag
import javax.inject.Inject

class DeleteTagGitAction @Inject constructor(
    private val jgit: JGit,
) : IDeleteTagGitAction {
    override suspend operator fun invoke(repositoryPath: String, tag: Tag) = jgit.provide(repositoryPath) { git ->
        git
            .tagDelete()
            .setTags(tag.name)
            .call()

        Unit
    }
}