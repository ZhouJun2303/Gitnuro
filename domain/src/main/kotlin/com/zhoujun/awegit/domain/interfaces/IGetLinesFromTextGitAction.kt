package com.zhoujun.awegit.domain.interfaces

interface IGetLinesFromTextGitAction {
    operator fun invoke(content: String): List<String>
}