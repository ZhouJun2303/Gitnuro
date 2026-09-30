package com.zhoujun.awegit.domain.usecases

import com.zhoujun.awegit.domain.UseCaseExecutor
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.errors.GenericError
import com.zhoujun.awegit.domain.interfaces.ICheckHasUncommittedChangesGitAction
import com.zhoujun.awegit.domain.interfaces.IGetStashListGitAction
import com.zhoujun.awegit.domain.interfaces.IPopStashGitAction
import com.zhoujun.awegit.domain.interfaces.IPullBranchGitAction
import com.zhoujun.awegit.domain.interfaces.IStageUntrackedFileGitAction
import com.zhoujun.awegit.domain.interfaces.IStashChangesGitAction
import com.zhoujun.awegit.domain.models.Branch
import com.zhoujun.awegit.domain.models.Commit
import com.zhoujun.awegit.domain.models.PullOptions
import com.zhoujun.awegit.domain.models.PullType
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.coVerifyOrder
import io.mockk.mockk
import kotlinx.coroutines.runBlocking
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class PullWithOptionsUseCaseTest {
    private val repo = "/repo"
    private val stashMessage = "AweGit: auto stash before pull"
    private val useCaseExecutor = mockk<UseCaseExecutor>(relaxed = true)
    private val checkHasUncommittedChanges = mockk<ICheckHasUncommittedChangesGitAction>()
    private val stageUntracked = mockk<IStageUntrackedFileGitAction>()
    private val stashChanges = mockk<IStashChangesGitAction>()
    private val getStashList = mockk<IGetStashListGitAction>()
    private val pullBranch = mockk<IPullBranchGitAction>()
    private val popStash = mockk<IPopStashGitAction>()
    private val remoteBranch = Branch(hash = "abc", name = "refs/remotes/origin/main", isLocal = false)

    private val useCase = PullWithOptionsUseCase(
        useCaseExecutor = useCaseExecutor,
        checkHasUncommittedChangesGitAction = checkHasUncommittedChanges,
        stageUntrackedFileGitAction = stageUntracked,
        stashChangesGitAction = stashChanges,
        getStashListGitAction = getStashList,
        pullBranchGitAction = pullBranch,
        popStashGitAction = popStash,
    )

    @Test
    fun `no local changes skips stash and uses the selected pull type`() = runBlocking {
        coEvery { checkHasUncommittedChanges(repo) } returns Either.Ok(false)
        coEvery { pullBranch(any(), any(), any(), any(), any()) } returns Either.Ok(false)

        val result = useCase.pull(repo, PullOptions(remoteBranch, rebase = true, stashAndReapply = true))

        assertTrue(result is Either.Ok)
        coVerify(exactly = 0) { stashChanges(any(), any()) }
        coVerify {
            pullBranch(repo, PullType.REBASE, false, remoteBranch, stashMessage)
        }

        coEvery { pullBranch(any(), any(), any(), any(), any()) } returns Either.Ok(false)
        useCase.pull(repo, PullOptions(remoteBranch, rebase = false, stashAndReapply = true))
        coVerify {
            pullBranch(repo, PullType.MERGE, false, remoteBranch, stashMessage)
        }
    }

    @Test
    fun `changes are stashed then reapplied after a clean pull`() = runBlocking {
        val stash = mockk<Commit>()
        coEvery { checkHasUncommittedChanges(repo) } returns Either.Ok(true)
        coEvery { stageUntracked(repo) } returns Either.Ok(Unit)
        coEvery { stashChanges(repo, stashMessage) } returns Either.Ok(Unit)
        coEvery { getStashList(repo) } returns Either.Ok(listOf(stash))
        coEvery { pullBranch(any(), any(), any(), any(), any()) } returns Either.Ok(false)
        coEvery { popStash(repo, stash) } returns Either.Ok(Unit)

        val result = useCase.pull(repo, PullOptions(remoteBranch, rebase = false, stashAndReapply = true))

        assertTrue(result is Either.Ok)
        coVerifyOrder {
            stageUntracked(repo)
            stashChanges(repo, stashMessage)
            pullBranch(repo, PullType.MERGE, false, remoteBranch, stashMessage)
            popStash(repo, stash)
        }
    }

    @Test
    fun `conflicts keep the stash and return an error naming it`() = runBlocking {
        val stash = mockk<Commit>()
        coEvery { checkHasUncommittedChanges(repo) } returns Either.Ok(true)
        coEvery { stageUntracked(repo) } returns Either.Ok(Unit)
        coEvery { stashChanges(repo, stashMessage) } returns Either.Ok(Unit)
        coEvery { getStashList(repo) } returns Either.Ok(listOf(stash))
        coEvery { pullBranch(any(), any(), any(), any(), any()) } returns Either.Ok(true)

        val result = useCase.pull(repo, PullOptions(remoteBranch, rebase = false, stashAndReapply = true))

        assertTrue(result is Either.Err)
        val message = ((result as Either.Err).error as GenericError).message
        assertTrue(message.contains(stashMessage))
        coVerify(exactly = 0) { popStash(any(), any()) }
    }

    @Test
    fun `failed pull restores the stash and returns the original error`() = runBlocking {
        val stash = mockk<Commit>()
        val error = GenericError("network down")
        coEvery { checkHasUncommittedChanges(repo) } returns Either.Ok(true)
        coEvery { stageUntracked(repo) } returns Either.Ok(Unit)
        coEvery { stashChanges(repo, stashMessage) } returns Either.Ok(Unit)
        coEvery { getStashList(repo) } returns Either.Ok(listOf(stash))
        coEvery { pullBranch(any(), any(), any(), any(), any()) } returns Either.Err(error)
        coEvery { popStash(repo, stash) } returns Either.Ok(Unit)

        val result = useCase.pull(repo, PullOptions(remoteBranch, rebase = false, stashAndReapply = true))

        assertEquals(error, (result as Either.Err).error)
        coVerify { popStash(repo, stash) }
    }

    @Test
    fun `stash checkbox off never stashes`() = runBlocking {
        coEvery { checkHasUncommittedChanges(repo) } returns Either.Ok(true)
        coEvery { pullBranch(any(), any(), any(), any(), any()) } returns Either.Ok(false)

        val result = useCase.pull(repo, PullOptions(remoteBranch, rebase = false, stashAndReapply = false))

        assertTrue(result is Either.Ok)
        coVerify(exactly = 0) { stageUntracked(any()) }
        coVerify(exactly = 0) { stashChanges(any(), any()) }
        coVerify(exactly = 0) { popStash(any(), any()) }
    }
}
