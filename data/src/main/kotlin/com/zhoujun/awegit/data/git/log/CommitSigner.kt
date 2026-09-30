package com.zhoujun.awegit.data.git.log

import org.eclipse.jgit.lib.CommitBuilder
import org.eclipse.jgit.lib.PersonIdent

fun interface CommitSigner {
    fun sign(builder: CommitBuilder, committer: PersonIdent)
}
