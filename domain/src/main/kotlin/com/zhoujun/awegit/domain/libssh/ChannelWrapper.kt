package com.zhoujun.awegit.domain.libssh

import com.zhoujun.awegit.Channel
import com.zhoujun.awegit.Session
import com.zhoujun.awegit.domain.exceptions.SshException
import com.zhoujun.awegit.domain.extensions.throwIfSshMessage
import com.zhoujun.awegit.domain.libssh.streams.SshChannelInputErrStream
import com.zhoujun.awegit.domain.libssh.streams.SshChannelInputStream
import com.zhoujun.awegit.domain.libssh.streams.SshChannelOutputStream
import java.util.concurrent.Semaphore

class ChannelWrapper internal constructor(sshSession: Session) {
    private val channel = Channel(sshSession)
        ?: throw SshException("Could not obtain the channel, this is likely a bug. Please file a report.")

    private var isClosed = false
    private var closeMutex = Semaphore(1)
    val outputStream = SshChannelOutputStream(channel)
    val inputStream = SshChannelInputStream(channel)
    val errorOutputStream = SshChannelInputErrStream(channel)

    fun openSession() {
        channel.openSession()
    }

    fun requestExec(commandName: String) {
        channel.requestExec(commandName).throwIfSshMessage()
    }

    fun isOpen(): Boolean {
        return channel.isOpen()
    }

    fun close() {
        closeMutex.acquire()
        try {
            if (!isClosed) {
                channel.closeChannel().throwIfSshMessage()
                channel.close()
                isClosed = true
            }
        } finally {
            closeMutex.release()
        }
    }
}