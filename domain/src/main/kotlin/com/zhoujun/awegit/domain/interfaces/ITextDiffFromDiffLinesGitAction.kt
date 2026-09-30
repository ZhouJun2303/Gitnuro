package com.zhoujun.awegit.domain.interfaces

import com.zhoujun.awegit.domain.models.Line

interface ITextDiffFromDiffLinesGitAction {
    operator fun invoke(lines: List<Line>): List<Line>
}