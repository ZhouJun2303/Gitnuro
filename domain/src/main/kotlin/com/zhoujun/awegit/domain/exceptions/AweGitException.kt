package com.zhoujun.awegit.domain.exceptions

abstract class AweGitException(message: String, cause: Exception? = null) : Exception(message, cause)