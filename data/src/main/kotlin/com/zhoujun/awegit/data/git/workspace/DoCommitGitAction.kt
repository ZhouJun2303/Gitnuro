package com.zhoujun.awegit.data.git.workspace

import com.zhoujun.awegit.common.use
import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.data.git.repository.GetRepositoryStateGitAction
import com.zhoujun.awegit.data.mappers.JGitCommitMapper
import com.zhoujun.awegit.data.mappers.JGitIdentityMapper
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GenericError
import com.zhoujun.awegit.domain.errors.GitError
import com.zhoujun.awegit.domain.errors.HookRejectionError
import com.zhoujun.awegit.domain.errors.bind
import com.zhoujun.awegit.domain.interfaces.IDoCommitGitAction
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.Identity
import org.eclipse.jgit.api.errors.AbortedByHookException
import java.io.ByteArrayOutputStream
import java.io.PrintStream
import javax.inject.Inject

private const val TAG = "DoCommitGitAction"

class DoCommitGitAction @Inject constructor(
    private val getRepositoryStateGitAction: GetRepositoryStateGitAction,
    private val commitMapper: JGitCommitMapper,
    private val identityMapper: JGitIdentityMapper,
    private val jgit: JGit,
) : IDoCommitGitAction {
    override suspend operator fun invoke(
        repositoryPath: String,
        message: String,
        amend: Boolean,
        author: Identity?,
    ): Either<Commit, GitError> = jgit.provide(
        repositoryPath,
        errorHandle = { ex ->
            if (ex is AbortedByHookException) {
//                val out = output.toString(Charsets.UTF_8)
//                printLog(TAG, out)

                // TODO Do we need to read the output as it was done before the refactor?
                HookRejectionError(ex.hookStdErr)
            } else {
                GenericError(ex.message.orEmpty())
            }
        }
    ) { git ->
        val state = getRepositoryStateGitAction(repositoryPath).bind()
        val isMerging = state.isMerging
        val output = ByteArrayOutputStream()
        val printStream = PrintStream(output, true, Charsets.UTF_8)

        use(output, printStream) {
            val commit = git
                .commit()
                .setMessage(message)
                .setAllowEmpty(amend || isMerging) // Only allow empty commits when amending
                .setAmend(amend)
                .setHookErrorStream(printStream)
                .setHookOutputStream(printStream)
                .run {
                    if (author != null) {
                        setAuthor(identityMapper.toData(author))
                    } else {
                        this
                    }
                }
                .call()

            commitMapper.toDomain(commit)
        }
    }
}