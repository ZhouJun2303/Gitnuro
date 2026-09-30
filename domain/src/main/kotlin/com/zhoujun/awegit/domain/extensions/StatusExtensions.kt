package com.zhoujun.awegit.domain.extensions

import org.eclipse.jgit.api.Status

fun Status.hasUntrackedChanges(): Boolean {
    return this.untracked.isNotEmpty()
}