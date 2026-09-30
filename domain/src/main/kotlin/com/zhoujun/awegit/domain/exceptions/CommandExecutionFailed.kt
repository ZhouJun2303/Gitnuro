package com.zhoujun.awegit.domain.exceptions

class CommandExecutionFailed(msg: String, cause: Exception) : AweGitException(msg, cause) {
}