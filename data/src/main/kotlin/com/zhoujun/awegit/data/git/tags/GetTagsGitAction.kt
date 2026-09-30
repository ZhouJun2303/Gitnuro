package com.zhoujun.awegit.data.git.tags

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.mappers.JGitTagMapper
import com.zhoujun.awegit.domain.interfaces.IGetTagsGitAction
import javax.inject.Inject

class GetTagsGitAction @Inject constructor(
    private val tagMapper: JGitTagMapper,
    private val jgit: JGit,
) : IGetTagsGitAction {
    override suspend operator fun invoke(repositoryPath: String) = jgit.provide(repositoryPath) { git ->
        git
            .tagList()
            .call()
            .mapNotNull { tag ->
                val tag = if (!tag.isPeeled) {
                    git.repository.refDatabase.peel(tag)
                } else {
                    tag
                }

                tagMapper.toDomain(tag)
            }
    }
}