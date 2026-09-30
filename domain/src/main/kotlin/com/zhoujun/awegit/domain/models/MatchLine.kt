package com.zhoujun.awegit.domain.models

import com.zhoujun.awegit.domain.DiffMatchPatch

data class MatchLine(val diffs: List<DiffMatchPatch.Diff>)