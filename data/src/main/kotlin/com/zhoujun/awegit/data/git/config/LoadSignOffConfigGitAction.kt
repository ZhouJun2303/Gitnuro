package com.zhoujun.awegit.data.git.config

import com.zhoujun.awegit.data.git.JGit
import com.zhoujun.awegit.domain.SignOffConstants
import com.zhoujun.awegit.domain.extensions.nullIfEmpty
import com.zhoujun.awegit.domain.interfaces.ILoadSignOffConfigGitAction
import com.zhoujun.awegit.domain.models.SignOffConfig
import org.eclipse.jgit.storage.file.FileBasedConfig
import java.io.File
import javax.inject.Inject


class LoadSignOffConfigGitAction @Inject constructor(
    private val jgit: JGit,
) : ILoadSignOffConfigGitAction {
    override suspend operator fun invoke(repositoryPath: String) = jgit.provide(repositoryPath) { git ->
        val configFile = File(git.repository.directory, LocalConfigConstants.CONFIG_FILE_NAME)
            .takeIf { it.exists() }
            ?: File(git.repository.directory, LocalConfigConstants.LEGACY_CONFIG_FILE_NAME)
                .takeIf { it.exists() }
            ?: File(git.repository.directory, LocalConfigConstants.CONFIG_FILE_NAME).apply { createNewFile() }

        val config = FileBasedConfig(configFile, git.repository.fs)
        config.load()

        val enabled = config.getBoolean(
            SignOffConstants.SECTION,
            null,
            SignOffConstants.FIELD_ENABLED,
            SignOffConstants.DEFAULT_SIGN_OFF_ENABLED
        )


        val format = config.getString(
            SignOffConstants.SECTION,
            null,
            SignOffConstants.FIELD_FORMAT
        )?.nullIfEmpty ?: SignOffConstants.DEFAULT_SIGN_OFF_FORMAT

        SignOffConfig(
            enabled,
            format,
            config.getStringList("awegit", null, "hiddenRef").toList(),
        )
    }
}
