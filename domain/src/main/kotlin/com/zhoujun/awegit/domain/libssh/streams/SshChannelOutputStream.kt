package com.zhoujun.awegit.domain.libssh.streams

import com.zhoujun.awegit.Channel
import com.zhoujun.awegit.domain.extensions.throwIfSshMessage
import java.io.OutputStream

class SshChannelOutputStream(private val sshChannel: Channel) : OutputStream() {
    override fun write(b: Int) {
        val byteArrayData = byteArrayOf(b.toByte())
        write(byteArrayData)
    }

    override fun write(b: ByteArray) {
        sshChannel.writeBytes(b).throwIfSshMessage()
    }

    override fun close() {
    }
}
